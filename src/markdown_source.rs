use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};

use crate::pager::PageSource;
use crate::render::{TextRenderer, TextSpan, paginate_spans_by_terminal_height};

/// Renders a markdown file as extracted, formatted text, paginated to fit the terminal.
///
/// There's no fixed page layout for a plain text file, so the whole document is one logical
/// page that then gets chunked to the current terminal height, the same way a docx without
/// any explicit page breaks does.
///
/// Headings, emphasis, code and lists are turned into [TextSpan]s carrying bold/italic/code
/// flags plus a few literal markers (heading `#`s, list bullets, blockquote `>`); whether that
/// ends up as ANSI styling or gets stripped to plain text is up to the `renderer` passed to
/// [MarkdownPageSource::load], mirroring `DocxPageSource`.
pub struct MarkdownPageSource {
    pages: Vec<Vec<TextSpan>>,
    renderer: Box<dyn TextRenderer>,
}

impl MarkdownPageSource {
    pub fn load(path: &str, renderer: Box<dyn TextRenderer>) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let pages = paginate_spans_by_terminal_height(vec![parse_markdown(&content)]);
        Ok(Self { pages, renderer })
    }
}

impl PageSource for MarkdownPageSource {
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

enum ListKind {
    Ordered(u64),
    Unordered,
}

/// Accumulates a GFM table's cells while parsing, so the whole thing can be laid out (column
/// widths, alignment, separator row) once [TagEnd::Table] is reached, instead of being emitted
/// row-by-row as it streams past (which is what produced the "everything on one line" bug --
/// `Event::SoftBreak` between source rows was rendered as a plain space).
struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Vec<TextSpan>>>,
    current_row: Vec<Vec<TextSpan>>,
    current_cell: Vec<TextSpan>,
}

/// Routes inline content (text, code, breaks) to the span buffer that's actually active:
/// the current table cell while inside a table, otherwise the top-level document spans.
fn active_spans<'a>(spans: &'a mut Vec<TextSpan>, table: &'a mut Option<TableState>) -> &'a mut Vec<TextSpan> {
    match table {
        Some(t) => &mut t.current_cell,
        None => spans,
    }
}

fn cell_plain_len(cell: &[TextSpan]) -> usize {
    cell.iter().map(|span| span.text.chars().count()).sum()
}

fn push_table_row(spans: &mut Vec<TextSpan>, row: Vec<Vec<TextSpan>>, widths: &[usize], alignments: &[Alignment]) {
    spans.push(TextSpan::plain("|"));
    for (i, cell) in row.into_iter().enumerate() {
        let width = widths.get(i).copied().unwrap_or(0);
        let pad = width.saturating_sub(cell_plain_len(&cell));
        let (left_pad, right_pad) = match alignments.get(i) {
            Some(Alignment::Right) => (pad, 0),
            Some(Alignment::Center) => (pad / 2, pad - pad / 2),
            _ => (0, pad),
        };

        spans.push(TextSpan::plain(format!(" {}", " ".repeat(left_pad))));
        spans.extend(cell);
        spans.push(TextSpan::plain(format!("{} |", " ".repeat(right_pad))));
    }
    spans.push(TextSpan::plain("\n"));
}

fn push_table_separator(spans: &mut Vec<TextSpan>, widths: &[usize]) {
    spans.push(TextSpan::plain("|"));
    for &width in widths {
        spans.push(TextSpan::plain(format!(" {} |", "-".repeat(width.max(1)))));
    }
    spans.push(TextSpan::plain("\n"));
}

fn render_table(alignments: Vec<Alignment>, rows: Vec<Vec<Vec<TextSpan>>>) -> Vec<TextSpan> {
    let column_count = alignments.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let mut widths = vec![0usize; column_count];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell_plain_len(cell));
        }
    }

    let mut spans = Vec::new();
    for (row_index, row) in rows.into_iter().enumerate() {
        push_table_row(&mut spans, row, &widths, &alignments);
        if row_index == 0 {
            push_table_separator(&mut spans, &widths);
        }
    }
    spans
}

fn parse_markdown(content: &str) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    let mut bold_depth = 0u32;
    let mut italic_depth = 0u32;
    let mut strikethrough_depth = 0u32;
    let mut blockquote_depth = 0u32;
    let mut link_depth = 0u32;
    let mut in_heading = false;
    let mut heading_level = 0u8;
    let mut in_code_block = false;
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut table: Option<TableState> = None;

    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for event in Parser::new_ext(content, options) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    in_heading = true;
                    heading_level = level as u8;
                    bold_depth += 1;
                    // H1 gets one space of padding on each side so its background banner
                    // (see StyledTextRenderer) doesn't hug the text edges. Kept as its own
                    // span (not part of the '#' marker) so it stays visible even though the
                    // marker itself is hidden in styled mode.
                    if heading_level == 1 {
                        spans.push(TextSpan {
                            heading_level,
                            ..TextSpan::plain(" ")
                        });
                    }
                    spans.push(TextSpan {
                        bold: true,
                        heading_level,
                        is_marker: true,
                        ..TextSpan::plain(format!("{} ", "#".repeat(heading_level as usize)))
                    });
                }
                Tag::Emphasis => italic_depth += 1,
                Tag::Strong => bold_depth += 1,
                Tag::Strikethrough => strikethrough_depth += 1,
                Tag::Link { .. } => link_depth += 1,
                Tag::BlockQuote(_) => {
                    blockquote_depth += 1;
                    spans.push(TextSpan {
                        is_marker: true,
                        ..TextSpan::plain("> ")
                    });
                }
                Tag::CodeBlock(_) => in_code_block = true,
                Tag::List(start) => {
                    if !list_stack.is_empty() {
                        // A nested list starts mid-item (right after the parent item's own
                        // text), so break to a new line before its first bullet.
                        spans.push(TextSpan::plain("\n"));
                    }
                    list_stack.push(match start {
                        Some(n) => ListKind::Ordered(n),
                        None => ListKind::Unordered,
                    });
                }
                Tag::Item => {
                    let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                    let marker = match list_stack.last_mut() {
                        Some(ListKind::Ordered(n)) => {
                            let marker = format!("{indent}{n}. ");
                            *n += 1;
                            marker
                        }
                        _ => format!("{indent}- "),
                    };
                    spans.push(TextSpan::plain(marker));
                }
                Tag::Table(alignments) => {
                    table = Some(TableState {
                        alignments,
                        rows: Vec::new(),
                        current_row: Vec::new(),
                        current_cell: Vec::new(),
                    });
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    if heading_level == 1 {
                        spans.push(TextSpan {
                            heading_level: 1,
                            ..TextSpan::plain(" ")
                        });
                    }
                    in_heading = false;
                    heading_level = 0;
                    bold_depth = bold_depth.saturating_sub(1);
                    spans.push(TextSpan::plain("\n\n"));
                }
                TagEnd::Paragraph => spans.push(TextSpan::plain("\n\n")),
                TagEnd::Emphasis => italic_depth = italic_depth.saturating_sub(1),
                TagEnd::Strong => bold_depth = bold_depth.saturating_sub(1),
                TagEnd::Strikethrough => strikethrough_depth = strikethrough_depth.saturating_sub(1),
                TagEnd::Link => link_depth = link_depth.saturating_sub(1),
                TagEnd::BlockQuote(_) => {
                    blockquote_depth = blockquote_depth.saturating_sub(1);
                    spans.push(TextSpan::plain("\n"));
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    spans.push(TextSpan::plain("\n"));
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    if list_stack.is_empty() {
                        // Only add spacing once we've exited every nesting level.
                        spans.push(TextSpan::plain("\n"));
                    }
                }
                TagEnd::Item => spans.push(TextSpan::plain("\n")),
                TagEnd::TableCell => {
                    if let Some(t) = table.as_mut() {
                        let cell = std::mem::take(&mut t.current_cell);
                        t.current_row.push(cell);
                    }
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    if let Some(t) = table.as_mut() {
                        let row = std::mem::take(&mut t.current_row);
                        t.rows.push(row);
                    }
                }
                TagEnd::Table => {
                    if let Some(t) = table.take() {
                        spans.extend(render_table(t.alignments, t.rows));
                        spans.push(TextSpan::plain("\n"));
                    }
                }
                _ => {}
            },
            Event::Text(text) => active_spans(&mut spans, &mut table).push(TextSpan {
                bold: bold_depth > 0,
                italic: italic_depth > 0,
                code: in_code_block,
                strikethrough: strikethrough_depth > 0,
                blockquote: blockquote_depth > 0,
                link: link_depth > 0,
                heading_level: if in_heading { heading_level } else { 0 },
                ..TextSpan::plain(text.into_string())
            }),
            Event::Code(text) => {
                let bold = bold_depth > 0;
                let italic = italic_depth > 0;
                let strikethrough = strikethrough_depth > 0;
                let blockquote = blockquote_depth > 0;
                let link = link_depth > 0;
                let sink = active_spans(&mut spans, &mut table);
                // Backticks are the literal markup, shown only in plain mode (see is_marker);
                // the code content itself keeps its `code` styling either way.
                sink.push(TextSpan {
                    is_marker: true,
                    ..TextSpan::plain("`")
                });
                sink.push(TextSpan {
                    bold,
                    italic,
                    code: true,
                    strikethrough,
                    blockquote,
                    link,
                    ..TextSpan::plain(text.into_string())
                });
                sink.push(TextSpan {
                    is_marker: true,
                    ..TextSpan::plain("`")
                });
            }
            Event::SoftBreak => active_spans(&mut spans, &mut table).push(TextSpan::plain(" ")),
            Event::HardBreak => active_spans(&mut spans, &mut table).push(TextSpan::plain("\n")),
            Event::Rule => spans.push(TextSpan::plain("───\n\n")),
            _ => {}
        }
    }

    spans
}

