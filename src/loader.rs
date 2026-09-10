use std::path::Path;

use image::RgbImage;
use pdfium_render::prelude::*;

use crate::Options;
use crate::docx_source::DocxPageSource;
use crate::pager::run_paged;
use crate::render::{
    AsciiRenderer, HalfBlockRenderer, PlainTextRenderer, Renderer, StyledTextRenderer, TextRenderer,
    current_terminal_size,
};
use crate::sources::{ImagePageSource, PdfImagePageSource, PdfTextPageSource};

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
/// (image file, PDF via pdfium, docx via docx-rs) actually handles a given path.
pub(crate) fn run(options: &Options) -> Result<(), String> {
    let extension = Path::new(&options.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_pdf = extension == "pdf";
    let is_docx = extension == "docx";

    if options.text && !is_pdf && !is_docx {
        return Err("--text can only be used with PDF or DOCX files".to_string());
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

    if options.text {
        return run_pdf_text(&options.path);
    }

    if is_pdf {
        return run_pdf_image(options);
    }

    run_static_image(options)
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

fn run_static_image(options: &Options) -> Result<(), String> {
    let renderer = build_renderer(options);
    let (terminal_width, terminal_height) = current_terminal_size();
    let source =
        ImagePageSource::load(options.path.clone(), renderer, terminal_width, terminal_height)?;
    run_paged(&source)
}

fn build_renderer(options: &Options) -> Box<dyn Renderer> {
    if options.ascii {
        Box::new(AsciiRenderer)
    } else {
        Box::new(HalfBlockRenderer)
    }
}

