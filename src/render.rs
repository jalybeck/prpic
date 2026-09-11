use std::fmt::Write;

use image::RgbImage;
use image::imageops::FilterType;
use terminal_size::{Height, Width, terminal_size};

// Light to dark. ASCII characters are reliably one terminal cell wide.
const CHARS: &[u8] = b" .:-=+*#%@";
const FALLBACK_TERMINAL_SIZE: (u32, u32) = (80, 24);

// A terminal cell is usually about twice as tall as it is wide.
const CELL_ASPECT_RATIO: f64 = 0.5;

/// Renders pixel data to a terminal-displayable string.
pub trait Renderer {
    fn render(&self, image: &RgbImage, terminal_width: u32, terminal_height: u32) -> String;
}

pub struct AsciiRenderer;

impl Renderer for AsciiRenderer {
    fn render(&self, image: &RgbImage, terminal_width: u32, terminal_height: u32) -> String {
        let (width, height) = image.dimensions();
        let (output_width, output_height) =
            ascii_output_dimensions(width, height, terminal_width, terminal_height);
        let resized =
            image::imageops::resize(image, output_width, output_height, FilterType::Triangle);

        let mut output = String::with_capacity((output_width as usize + 1) * output_height as usize);

        for pixel_row in resized.rows() {
            for pixel in pixel_row {
                let [r, g, b] = pixel.0;

                // Rec. 709 luminance
                let luminance = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
                let darkness = 1.0 - luminance / 255.0;
                let idx = (darkness * (CHARS.len() - 1) as f32).round() as usize;
                output.push(CHARS[idx] as char);
            }
            output.push('\n');
        }

        output
    }
}

pub struct HalfBlockRenderer;

impl Renderer for HalfBlockRenderer {
    fn render(&self, image: &RgbImage, terminal_width: u32, terminal_height: u32) -> String {
        let (width, height) = image.dimensions();
        let (output_width, output_height) =
            half_block_pixel_dimensions(width, height, terminal_width, terminal_height);
        let resized =
            image::imageops::resize(image, output_width, output_height, FilterType::Lanczos3);
        let terminal_rows = output_height.div_ceil(2);
        let mut output = String::with_capacity(
            output_width as usize * terminal_rows as usize * 40 + terminal_rows as usize * 5,
        );

        for y in (0..output_height).step_by(2) {
            for x in 0..output_width {
                let [top_r, top_g, top_b] = resized.get_pixel(x, y).0;
                let [bottom_r, bottom_g, bottom_b] =
                    resized.get_pixel(x, (y + 1).min(output_height - 1)).0;

                // The foreground paints the upper half and the background the lower half.
                write!(
                    output,
                    "\x1b[38;2;{top_r};{top_g};{top_b}m\x1b[48;2;{bottom_r};{bottom_g};{bottom_b}m▀"
                )
                .expect("writing to a String cannot fail");
            }
            output.push_str("\x1b[0m\n");
        }

        output
    }
}

pub fn current_terminal_size() -> (u32, u32) {
    terminal_size()
        .map(|(Width(width), Height(height))| (u32::from(width), u32::from(height)))
        .unwrap_or(FALLBACK_TERMINAL_SIZE)
}

/// A run of extracted document text with optional formatting, independent of which file
/// format (docx, PDF, markdown, ...) it was extracted from.
pub struct TextSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub code: bool,
    pub strikethrough: bool,
    /// Quoted text (markdown `>`): styled dim/italic, no visual equivalent needed for the
    /// literal `>` marker itself (see `is_marker`).
    pub blockquote: bool,
    /// Link text (markdown `[text](url)`): styled like a typical terminal hyperlink. The
    /// destination URL itself is intentionally dropped, only the visible text is kept.
    pub link: bool,
    /// 0 for non-heading text, otherwise the markdown heading level (1 = `#`, 2 = `##`, ...).
    pub heading_level: u8,
    /// Literal markup text (`#`, `` ` ``, `>`): printed in plain text mode, but skipped when
    /// styled since color/formatting already conveys the meaning.
    pub is_marker: bool,
}

impl TextSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            underline: false,
            code: false,
            strikethrough: false,
            blockquote: false,
            link: false,
            heading_level: 0,
            is_marker: false,
        }
    }
}

/// Renders extracted document text (as opposed to rasterized pixels) to a terminal string.
/// The text/pixel split mirrors [Renderer]: sources hand over [TextSpan]s and stay unaware
/// of whether the result ends up styled or plain.
pub trait TextRenderer {
    fn render(&self, spans: &[TextSpan]) -> String;
}

/// Keeps bold/italic/underline/code formatting via ANSI codes. Used for the default
/// (non `--text`) rendering of text-based documents.
pub struct StyledTextRenderer;

impl TextRenderer for StyledTextRenderer {
    fn render(&self, spans: &[TextSpan]) -> String {
        let mut output = String::new();

        for span in spans {
            if span.text.is_empty() || span.is_marker {
                continue;
            }

            let mut codes = Vec::new();
            match span.heading_level {
                // H1 gets a full banner (bold white-on-blue) to anchor the top of a document.
                1 => codes.extend(["1", "97", "44"]),
                // H2 stays inline but still stands out via a bold accent color.
                2 => codes.extend(["1", "96"]),
                _ if span.bold => codes.push("1"),
                _ => {}
            }
            if span.italic || span.blockquote {
                codes.push("3");
            }
            if span.underline || span.link {
                codes.push("4");
            }
            if span.code {
                codes.push("2"); // faint, to set code apart from surrounding prose
            }
            if span.strikethrough {
                codes.push("9");
            }
            if span.blockquote {
                codes.push("90"); // dim gray, sets quoted text apart from surrounding prose
            }
            if span.link {
                codes.push("94"); // bright blue, mirrors the usual hyperlink color
            }

            if codes.is_empty() {
                output.push_str(&span.text);
            } else {
                write!(output, "\x1b[{}m{}\x1b[0m", codes.join(";"), span.text)
                    .expect("writing to a String cannot fail");
            }
        }

        output
    }
}

/// Strips all styling down to plain text. Used for `--text` mode.
pub struct PlainTextRenderer;

impl TextRenderer for PlainTextRenderer {
    fn render(&self, spans: &[TextSpan]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// Splits already-paged spans further into chunks of at most one terminal height's worth of
/// lines, leaving one line free for the "-- MORE --" prompt itself. Shared by every text-based
/// [crate::pager::PageSource] (docx, markdown, plain text) so long content pages correctly
/// even when the source format has no page boundaries of its own.
pub fn paginate_spans_by_terminal_height(pages: Vec<Vec<TextSpan>>) -> Vec<Vec<TextSpan>> {
    let (_, terminal_height) = current_terminal_size();
    let lines_per_page = (terminal_height as usize).saturating_sub(1).max(1);

    let mut result = Vec::new();

    for page in pages {
        let mut chunk = Vec::new();
        let mut lines_in_chunk = 0;

        for span in page {
            for (i, part) in span.text.split('\n').enumerate() {
                if i > 0 {
                    lines_in_chunk += 1;
                    if lines_in_chunk >= lines_per_page {
                        result.push(std::mem::take(&mut chunk));
                        lines_in_chunk = 0;
                    } else {
                        chunk.push(TextSpan::plain("\n"));
                    }
                }

                if !part.is_empty() {
                    chunk.push(TextSpan {
                        text: part.to_string(),
                        bold: span.bold,
                        italic: span.italic,
                        underline: span.underline,
                        code: span.code,
                        strikethrough: span.strikethrough,
                        blockquote: span.blockquote,
                        link: span.link,
                        heading_level: span.heading_level,
                        is_marker: span.is_marker,
                    });
                }
            }
        }

        result.push(chunk);
    }

    if result.is_empty() {
        result.push(Vec::new());
    }

    result
}



fn ascii_output_dimensions(
    image_width: u32,
    image_height: u32,
    terminal_width: u32,
    terminal_height: u32,
) -> (u32, u32) {
    let max_width = terminal_width.max(1);
    // Keep the last terminal row free so the final newline does not scroll the image.
    let max_height = terminal_height.saturating_sub(1).max(1);

    let width_scale = max_width as f64 / image_width as f64;
    let height_scale = max_height as f64 / (image_height as f64 * CELL_ASPECT_RATIO);
    let scale = width_scale.min(height_scale).min(1.0);

    let output_width = (image_width as f64 * scale)
        .round()
        .clamp(1.0, max_width as f64) as u32;
    let output_height = (image_height as f64 * scale * CELL_ASPECT_RATIO)
        .round()
        .clamp(1.0, max_height as f64) as u32;

    (output_width, output_height)
}

fn half_block_pixel_dimensions(
    image_width: u32,
    image_height: u32,
    terminal_width: u32,
    terminal_height: u32,
) -> (u32, u32) {
    let max_width = terminal_width.max(1);
    let max_pixel_height = terminal_height.saturating_sub(1).max(1) * 2;

    let width_scale = max_width as f64 / image_width as f64;
    let height_scale = max_pixel_height as f64 / image_height as f64;
    let scale = width_scale.min(height_scale).min(1.0);

    let output_width = (image_width as f64 * scale)
        .round()
        .clamp(1.0, max_width as f64) as u32;
    let output_height = (image_height as f64 * scale)
        .round()
        .clamp(1.0, max_pixel_height as f64) as u32;

    (output_width, output_height)
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::{Renderer, ascii_output_dimensions, half_block_pixel_dimensions};

    #[test]
    fn ascii_landscape_image_fits_both_terminal_dimensions() {
        assert_eq!(ascii_output_dimensions(400, 300, 80, 24), (61, 23));
    }

    #[test]
    fn ascii_wide_image_uses_terminal_width() {
        assert_eq!(ascii_output_dimensions(1000, 100, 80, 24), (80, 4));
    }

    #[test]
    fn ascii_portrait_image_uses_terminal_height() {
        assert_eq!(ascii_output_dimensions(100, 1000, 80, 24), (5, 23));
    }

    #[test]
    fn ascii_small_image_is_not_enlarged() {
        assert_eq!(ascii_output_dimensions(20, 10, 80, 24), (20, 5));
    }

    #[test]
    fn half_blocks_double_the_available_vertical_resolution() {
        assert_eq!(half_block_pixel_dimensions(400, 300, 80, 24), (61, 46));
    }

    #[test]
    fn half_block_uses_the_top_and_bottom_pixel_colors() {
        let mut image = RgbImage::new(1, 2);
        image.put_pixel(0, 0, Rgb([1, 2, 3]));
        image.put_pixel(0, 1, Rgb([4, 5, 6]));

        assert_eq!(
            super::HalfBlockRenderer.render(&image, 1, 2),
            "\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀\x1b[0m\n"
        );
    }
}
