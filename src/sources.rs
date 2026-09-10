use image::RgbImage;
use pdfium_render::prelude::*;

use crate::loader::{FileLoader, Loader, PDF_RENDER_TARGET_WIDTH};
use crate::pager::PageSource;
use crate::render::Renderer;

/// A single static image treated as a one-page source. It runs through the same
/// paging machinery as multi-page content, it just never shows a "--MORE--" prompt.
pub struct ImagePageSource {
    image: RgbImage,
    renderer: Box<dyn Renderer>,
    terminal_width: u32,
    terminal_height: u32,
}

impl ImagePageSource {
    pub fn load(
        path: String,
        renderer: Box<dyn Renderer>,
        terminal_width: u32,
        terminal_height: u32,
    ) -> Result<Self, String> {
        let image = FileLoader { path }.load()?;
        Ok(Self {
            image,
            renderer,
            terminal_width,
            terminal_height,
        })
    }
}

impl PageSource for ImagePageSource {
    fn page_count(&self) -> Result<usize, String> {
        Ok(1)
    }

    fn render_page(&self, _index: usize) -> Result<String, String> {
        Ok(self
            .renderer
            .render(&self.image, self.terminal_width, self.terminal_height))
    }
}

/// Renders each page of a PDF as a rasterized image, one "page" per PDF page.
pub struct PdfImagePageSource<'a> {
    document: PdfDocument<'a>,
    renderer: Box<dyn Renderer>,
    terminal_width: u32,
    terminal_height: u32,
}

impl<'a> PdfImagePageSource<'a> {
    pub fn new(
        document: PdfDocument<'a>,
        renderer: Box<dyn Renderer>,
        terminal_width: u32,
        terminal_height: u32,
    ) -> Self {
        Self {
            document,
            renderer,
            terminal_width,
            terminal_height,
        }
    }
}

impl<'a> PageSource for PdfImagePageSource<'a> {
    fn page_count(&self) -> Result<usize, String> {
        Ok(self.document.pages().len() as usize)
    }

    fn render_page(&self, index: usize) -> Result<String, String> {
        let page = self
            .document
            .pages()
            .get(index as PdfPageIndex)
            .map_err(|e| format!("Failed to load PDF page {}: {e}", index + 1))?;

        let render_config = PdfRenderConfig::new().set_target_width(PDF_RENDER_TARGET_WIDTH);

        let image = page
            .render_with_config(&render_config)
            .map_err(|e| format!("Failed to render PDF page {}: {e}", index + 1))?
            .as_image()
            .map_err(|e| format!("Failed to convert PDF page {} to image: {e}", index + 1))?
            .into_rgb8();

        Ok(self
            .renderer
            .render(&image, self.terminal_width, self.terminal_height))
    }
}

/// Extracts the real text content of each PDF page instead of rasterizing it, so dense
/// body text stays legible regardless of terminal resolution.
pub struct PdfTextPageSource<'a> {
    document: PdfDocument<'a>,
}

impl<'a> PdfTextPageSource<'a> {
    pub fn new(document: PdfDocument<'a>) -> Self {
        Self { document }
    }
}

impl<'a> PageSource for PdfTextPageSource<'a> {
    fn page_count(&self) -> Result<usize, String> {
        Ok(self.document.pages().len() as usize)
    }

    fn render_page(&self, index: usize) -> Result<String, String> {
        let page = self
            .document
            .pages()
            .get(index as PdfPageIndex)
            .map_err(|e| format!("Failed to load PDF page {}: {e}", index + 1))?;

        Ok(page
            .text()
            .map_err(|e| format!("Failed to read text on PDF page {}: {e}", index + 1))?
            .all())
    }
}
