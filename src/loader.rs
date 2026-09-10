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

