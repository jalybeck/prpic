use std::path::Path;

use image::RgbImage;
use pdfium_render::prelude::*;

use crate::Options;
use crate::docx_source::DocxPageSource;
use crate::markdown_source::MarkdownPageSource;
use crate::pager::run_paged;
use crate::render::{
    AsciiRenderer, HalfBlockRenderer, PlainTextRenderer, Renderer, StyledTextRenderer, TextRenderer,
    current_terminal_size,
};
use crate::sources::{ImagePageSource, PdfImagePageSource, PdfTextPageSource};
use crate::text_source::TextFilePageSource;

/// Loads image data from some source, always producing pixel data.
pub trait Loader {
    fn load(&self) -> Result<RgbImage, String>;
}

pub struct FileLoader {
    pub path: String,
}

impl Loader for FileLoader {
    fn load(&self) -> Result<RgbImage, String> {
        image::open(&self.path)
            .map(|img| img.to_rgb8())
            .map_err(|e| format!("Failed to load image: {e}"))
    }
}

// build.rs copies this next to the compiled binary; release packages ship it the same way.
#[cfg(windows)]
const PDFIUM_LIBRARY_FILE_NAME: &str = "pdfium.dll";
#[cfg(target_os = "linux")]
const PDFIUM_LIBRARY_FILE_NAME: &str = "libpdfium.so";


#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn bind_pdfium() -> Result<Pdfium, String> {
    let library_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(PDFIUM_LIBRARY_FILE_NAME)))
        .filter(|path| path.exists());

    if let Some(library_path) = library_path {
        return Pdfium::bind_to_library(&library_path)
            .map(Pdfium::new)
            .map_err(|e| format!("Failed to load pdfium library at {}: {e}", library_path.display()));
    }

    Pdfium::bind_to_system_library().map(Pdfium::new).map_err(|e| {
        format!(
            "Failed to load a pdfium library: {e}\n\
             Expected to find {PDFIUM_LIBRARY_FILE_NAME} next to the prpic executable, \
             or a system-installed pdfium library.\n\
             Get one here: https://github.com/bblanchon/pdfium-binaries/releases"
        )
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn bind_pdfium() -> Result<Pdfium, String> {
    Pdfium::bind_to_system_library().map(Pdfium::new).map_err(|e| {
        format!(
            "Failed to load a system Pdfium library: {e}\n\
             Install one for your platform: https://github.com/bblanchon/pdfium-binaries/releases"
        )
    })
}

// Wide enough to keep detail after the renderer downsamples to terminal size.
pub(crate) const PDF_RENDER_TARGET_WIDTH: i32 = 1600;

/// Resolves `options` to the right content source and drives it through the pager.
///
/// This is the single entry point `main` calls into; it hides which concrete loader
/// (image file, PDF via pdfium, docx via docx-rs, markdown, plain text) actually handles a
/// given path.
pub(crate) fn run(options: &Options) -> Result<(), String> {
    let extension = Path::new(&options.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_pdf = extension == "pdf";
    let is_docx = extension == "docx";
    let is_md = extension == "md" || extension == "markdown";
    let is_txt = extension == "txt";

    if options.text && !is_pdf && !is_docx && !is_md {
        return Err("--text can only be used with PDF, DOCX, or Markdown files".to_string());
    }

    if is_docx {
        let renderer: Box<dyn TextRenderer> = if options.text {
            Box::new(PlainTextRenderer)
        } else {
            Box::new(StyledTextRenderer)
        };
        let source = DocxPageSource::load(&options.path, renderer)?;
        return run_paged(&source);
    }

    if is_md {
        let renderer: Box<dyn TextRenderer> = if options.text {
            Box::new(PlainTextRenderer)
        } else {
            Box::new(StyledTextRenderer)
        };
        let source = MarkdownPageSource::load(&options.path, renderer)?;
        return run_paged(&source);
    }

    if is_txt {
        let source = TextFilePageSource::load(&options.path)?;
        return run_paged(&source);
    }

    if options.text {
        return run_pdf_text(&options.path);
    }

    if is_pdf {
        return run_pdf_image(options);
    }

    run_image_or_text(options)
}

fn run_pdf_text(path: &str) -> Result<(), String> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("Failed to load PDF: {e}"))?;
    let source = PdfTextPageSource::new(document);
    run_paged(&source)
}

fn run_pdf_image(options: &Options) -> Result<(), String> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&options.path, None)
        .map_err(|e| format!("Failed to load PDF: {e}"))?;

    let renderer = build_renderer(options);
    let (terminal_width, terminal_height) = current_terminal_size();
    let source = PdfImagePageSource::new(document, renderer, terminal_width, terminal_height);
    run_paged(&source)
}

/// Handles any path whose extension isn't one of the explicitly known ones above.
///
/// Rather than requiring every text-ish extension (`.rs`, `.json`, `.log`, ...) to be listed,
/// this tries to load the file as an image first (the `image` crate sniffs actual file
/// signatures, not extensions). If that fails, it falls back to the same binary-detection
/// heuristic `trawl` uses to skip binary files when searching — but inverted: if the file
/// does *not* look binary, it's printed as plain text instead of surfacing the image error.
fn run_image_or_text(options: &Options) -> Result<(), String> {
    let renderer = build_renderer(options);
    let (terminal_width, terminal_height) = current_terminal_size();

    match ImagePageSource::load(options.path.clone(), renderer, terminal_width, terminal_height) {
        Ok(source) => run_paged(&source),
        Err(image_error) => {
            if looks_binary(&options.path) {
                Err(image_error)
            } else {
                let source = TextFilePageSource::load(&options.path)?;
                run_paged(&source)
            }
        }
    }
}

// Mirrors trawl's git-style binary detection: a BOM always means text (BOM'd UTF-16/32 text
// legitimately contains many NUL bytes), otherwise a NUL byte anywhere in the first buffered
// chunk marks the file as binary.
fn looks_binary(path: &str) -> bool {
    use std::io::BufRead;

    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = std::io::BufReader::new(file);
    let Ok(peek) = reader.fill_buf() else {
        return false;
    };

    let has_bom = peek.starts_with(&[0xEF, 0xBB, 0xBF])
        || peek.starts_with(&[0xFF, 0xFE])
        || peek.starts_with(&[0xFE, 0xFF])
        || peek.starts_with(&[0x00, 0x00, 0xFE, 0xFF]);

    !has_bom && peek.contains(&0)
}

fn build_renderer(options: &Options) -> Box<dyn Renderer> {
    if options.ascii {
        Box::new(AsciiRenderer)
    } else {
        Box::new(HalfBlockRenderer)
    }
}

