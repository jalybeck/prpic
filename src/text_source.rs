use crate::pager::PageSource;
use crate::render::{PlainTextRenderer, TextRenderer, TextSpan, paginate_spans_by_terminal_height};

/// Renders a plain text file, paginated to fit the terminal.
///
/// A plain text file has no formatting to preserve or strip, so unlike [crate::docx_source]
/// and [crate::markdown_source] this always renders through [PlainTextRenderer] — there's no
/// styled/`--text` distinction to make.
pub struct TextFilePageSource {
    pages: Vec<Vec<TextSpan>>,
}

impl TextFilePageSource {
    pub fn load(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let pages = paginate_spans_by_terminal_height(vec![vec![TextSpan::plain(content)]]);
        Ok(Self { pages })
    }
}

impl PageSource for TextFilePageSource {
    fn page_count(&self) -> Result<usize, String> {
        Ok(self.pages.len())
    }

    fn render_page(&self, index: usize) -> Result<String, String> {
        self.pages
            .get(index)
            .map(|spans| PlainTextRenderer.render(spans))
            .ok_or_else(|| format!("No such page: {}", index + 1))
    }
}
