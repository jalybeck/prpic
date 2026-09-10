use std::io::{self, Write};

use terminal_size::terminal_size;

/// Something that can be rendered to the terminal one "page" at a time.
///
/// This is the only thing a new multi-page content type (PDF pages, a future Word
/// document backend, etc.) needs to implement to get "--MORE--" paging for free from
/// [run_paged]. Single-page content (a plain image) can implement this too, reporting
/// a page count of 1; no prompt will ever be shown for it.
pub trait PageSource {
    /// Total number of pages available.
    fn page_count(&self) -> Result<usize, String>;

    /// Renders a single page (0-indexed) to a terminal-displayable string.
    fn render_page(&self, index: usize) -> Result<String, String>;
}

/// Prints every page of `source` in turn, pausing with a "--MORE--" prompt between
/// pages so the user can read one page at a time before continuing.
///
/// Paging only prompts when stdout looks like an interactive terminal; when the output
/// is redirected or piped, every page is printed back to back with no prompts.
pub fn run_paged(source: &dyn PageSource) -> Result<(), String> {
    let page_count = source.page_count()?;
    let interactive = terminal_size().is_some();

    for index in 0..page_count {
        print!("{}", source.render_page(index)?);
        io::stdout().flush().ok();

        let is_last_page = index + 1 == page_count;
        if !is_last_page && interactive && !prompt_more(index + 1, page_count) {
            break;
        }
    }

    Ok(())
}

/// Shows a "--MORE--" prompt and waits for the user to continue or quit.
/// Returns false if the user asked to stop.
fn prompt_more(pages_shown: usize, page_count: usize) -> bool {
    // Reverse video, bold: makes the prompt stand out from the page content above it.
    print!("\n\x1b[1;7m-- MORE ({pages_shown}/{page_count}) -- Enter to continue, q to quit --\x1b[0m");
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    !input.trim().eq_ignore_ascii_case("q")
}

#[cfg(test)]
mod tests {
    use super::PageSource;

    struct FixedPages(Vec<&'static str>);

    impl PageSource for FixedPages {
        fn page_count(&self) -> Result<usize, String> {
            Ok(self.0.len())
        }

        fn render_page(&self, index: usize) -> Result<String, String> {
            Ok(self.0[index].to_string())
        }
    }

    #[test]
    fn single_page_source_reports_exactly_one_page() {
        let source = FixedPages(vec!["only page"]);

        assert_eq!(source.page_count().unwrap(), 1);
        assert_eq!(source.render_page(0).unwrap(), "only page");
    }

    #[test]
    fn pages_are_addressable_by_index_in_order() {
        let source = FixedPages(vec!["first", "second", "third"]);

        assert_eq!(source.render_page(0).unwrap(), "first");
        assert_eq!(source.render_page(1).unwrap(), "second");
        assert_eq!(source.render_page(2).unwrap(), "third");
    }
}
