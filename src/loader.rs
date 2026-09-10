use image::RgbImage;
use pdfium_render::prelude::*;

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

// Wide enough to keep detail after the renderer downsamples to terminal size.
const PDF_RENDER_TARGET_WIDTH: i32 = 1600;

// build.rs copies this next to the compiled binary; release packages ship it the same way.
#[cfg(windows)]
const PDFIUM_LIBRARY_FILE_NAME: &str = "pdfium.dll";
#[cfg(target_os = "linux")]
const PDFIUM_LIBRARY_FILE_NAME: &str = "libpdfium.so";

#[cfg(any(windows, target_os = "linux"))]
fn bind_pdfium() -> Result<Pdfium, String> {
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
fn bind_pdfium() -> Result<Pdfium, String> {
    Pdfium::bind_to_system_library().map(Pdfium::new).map_err(|e| {
        format!(
            "Failed to load a system Pdfium library: {e}\n\
             Install one for your platform: https://github.com/bblanchon/pdfium-binaries/releases"
        )
    })
}

pub struct PdfLoader {
    pub path: String,
}

impl Loader for PdfLoader {
    fn load(&self) -> Result<RgbImage, String> {
        let pdfium = bind_pdfium()?;
        let document = pdfium
            .load_pdf_from_file(&self.path, None)
            .map_err(|e| format!("Failed to load PDF: {e}"))?;
        let page = document
            .pages()
            .first()
            .map_err(|e| format!("PDF has no pages: {e}"))?;

        let render_config = PdfRenderConfig::new().set_target_width(PDF_RENDER_TARGET_WIDTH);

        let image = page
            .render_with_config(&render_config)
            .map_err(|e| format!("Failed to render PDF page: {e}"))?
            .as_image()
            .map_err(|e| format!("Failed to convert PDF page to image: {e}"))?;

        Ok(image.into_rgb8())
    }
}

/// Extracts the real text content of a PDF's first page instead of rasterizing it,
/// so dense body text stays legible regardless of terminal resolution.
pub fn extract_pdf_text(path: &str) -> Result<String, String> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page = document
        .pages()
        .first()
        .map_err(|e| format!("PDF has no pages: {e}"))?;

    Ok(page
        .text()
        .map_err(|e| format!("Failed to read PDF text: {e}"))?
        .all())
}

