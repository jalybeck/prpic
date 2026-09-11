use docx_rs::{
    Break, BreakType, DocumentChild, InsertChild, ParagraphChild, Run, RunChild, SectionChild,
};

use crate::pager::PageSource;
use crate::render::{TextRenderer, TextSpan, paginate_spans_by_terminal_height};

/// Renders a .docx document as extracted text, paginated to fit the terminal.
///
/// Unlike a PDF, a .docx has no fixed page layout in the file itself — pagination is only
/// computed when something lays the text out for a specific page size and font. Without an
/// actual layout engine, explicit page breaks (Ctrl+Enter in Word) are the only page boundary
/// we can read directly from the file, so most documents have none. Each of those hard-break
/// pages is additionally chunked to the current terminal height, so long documents still pause
/// with "-- MORE --" between screens.
///
/// Extracted bold/italic/underline formatting is preserved as [TextSpan]s; whether that ends
/// up as ANSI styling or gets stripped to plain text is entirely up to the `renderer` passed
/// to [DocxPageSource::load], mirroring how `ImagePageSource` picks its pixel `Renderer`.
pub struct DocxPageSource {
    pages: Vec<Vec<TextSpan>>,
    renderer: Box<dyn TextRenderer>,
}

impl DocxPageSource {
    pub fn load(path: &str, renderer: Box<dyn TextRenderer>) -> Result<Self, String> {
        Ok(Self {
            pages: read_pages(path)?,
            renderer,
        })
    }
}

impl PageSource for DocxPageSource {
    fn page_count(&self) -> Result<usize, String> {
        Ok(self.pages.len())
    }

    fn render_page(&self, index: usize) -> Result<String, String> {
        self.pages
            .get(index)
            .map(|spans| self.renderer.render(spans))
            .ok_or_else(|| format!("No such page: {}", index + 1))
    }
}

fn read_pages(path: &str) -> Result<Vec<Vec<TextSpan>>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
    let docx = docx_rs::read_docx(&bytes).map_err(|e| format!("Failed to parse {path}: {e:?}"))?;

    let mut pages = Vec::new();
    let mut current = Vec::new();

    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                append_children(p.children(), &mut pages, &mut current)
            }
            DocumentChild::Section(s) => {
                for child in s.children() {
                    if let SectionChild::Paragraph(p) = child {
                        append_children(p.children(), &mut pages, &mut current);
                    }
                }
            }
            _ => {}
        }
    }

    pages.push(current);
    Ok(paginate_spans_by_terminal_height(pages))
}

fn append_children(
    children: &[ParagraphChild],
    pages: &mut Vec<Vec<TextSpan>>,
    current: &mut Vec<TextSpan>,
) {
    for child in children {
        match child {
            ParagraphChild::Run(run) => append_run(run, pages, current),
            ParagraphChild::Hyperlink(link) => append_children(&link.children, pages, current),
            ParagraphChild::Insert(insert) => {
                for child in &insert.children {
                    if let InsertChild::Run(run) = child {
                        append_run(run, pages, current);
                    }
                }
            }
            _ => {}
        }
    }
    current.push(TextSpan::plain("\n"));
}

fn append_run(run: &Run, pages: &mut Vec<Vec<TextSpan>>, current: &mut Vec<TextSpan>) {
    let page_break = Break::new(BreakType::Page);
    let mut text = String::new();

    for child in &run.children {
        match child {
            RunChild::Text(t) => text.push_str(&t.text),
            RunChild::Tab(_) | RunChild::PTab(_) => text.push('\t'),
            RunChild::Break(b) if *b == page_break => {
                push_run_text(&text, run, current);
                pages.push(std::mem::take(current));
                text.clear();
            }
            RunChild::Break(_) | RunChild::CarriageReturn(_) => text.push('\n'),
            _ => {}
        }
    }

    push_run_text(&text, run, current);
}

fn push_run_text(text: &str, run: &Run, current: &mut Vec<TextSpan>) {
    if text.is_empty() {
        return;
    }

    current.push(TextSpan {
        text: text.to_string(),
        bold: run.run_property.bold.is_some(),
        italic: run.run_property.italic.is_some(),
        underline: run.run_property.underline.is_some(),
        code: false,
        strikethrough: false,
        blockquote: false,
        link: false,
        heading_level: 0,
        is_marker: false,
    });
}

