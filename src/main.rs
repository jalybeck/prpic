mod loader;
mod pager;
mod render;
mod sources;

use std::env;
use std::path::Path;

use loader::bind_pdfium;
use pager::run_paged;
use render::{AsciiRenderer, HalfBlockRenderer, Renderer, current_terminal_size};
use sources::{ImagePageSource, PdfImagePageSource, PdfTextPageSource};

#[derive(Debug, PartialEq)]
struct Options {
    path: String,
    ascii: bool,
    text: bool,
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut path = None;
    let mut ascii = false;
    let mut text = false;

    for arg in args {
        match arg.as_str() {
            "--ascii" => ascii = true,
            "--text" => text = true,
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ if path.is_none() => path = Some(arg),
            _ => return Err("Only one image file can be given".to_string()),
        }
    }

    path.map(|path| Options { path, ascii, text })
        .ok_or_else(|| "Image file is required".to_string())
}

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "prpic".to_string());
    let options = parse_options(args).unwrap_or_else(|error| {
        eprintln!("{error}");
        eprintln!("Usage: {program} [--ascii] [--text] <image_file>");
        std::process::exit(1);
    });

    let is_pdf = Path::new(&options.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));

    if options.text && !is_pdf {
        eprintln!("--text can only be used with PDF files");
        std::process::exit(1);
    }

    let result = if options.text {
        run_pdf_text(&options.path)
    } else if is_pdf {
        run_pdf_image(&options)
    } else {
        run_static_image(&options)
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
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


#[cfg(test)]
mod tests {
    use super::parse_options;

    #[test]
    fn ascii_flag_can_be_before_or_after_the_path() {
        let before = parse_options(["--ascii".to_string(), "photo.jpg".to_string()]).unwrap();
        let after = parse_options(["photo.jpg".to_string(), "--ascii".to_string()]).unwrap();

        assert_eq!(before, after);
        assert!(before.ascii);
    }

    #[test]
    fn color_is_the_default_mode() {
        let options = parse_options(["photo.jpg".to_string()]).unwrap();

        assert!(!options.ascii);
    }

    #[test]
    fn text_flag_is_off_by_default() {
        let options = parse_options(["photo.jpg".to_string()]).unwrap();

        assert!(!options.text);
    }

    #[test]
    fn text_flag_can_be_combined_with_a_path() {
        let options = parse_options(["--text".to_string(), "document.pdf".to_string()]).unwrap();

        assert!(options.text);
    }
}
