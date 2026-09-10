mod loader;
mod render;

use std::env;
use std::path::Path;

use loader::{FileLoader, Loader, PdfLoader};
use render::{AsciiRenderer, HalfBlockRenderer, Renderer, current_terminal_size};

#[derive(Debug, PartialEq)]
struct Options {
    path: String,
    ascii: bool,
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut path = None;
    let mut ascii = false;

    for arg in args {
        match arg.as_str() {
            "--ascii" => ascii = true,
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ if path.is_none() => path = Some(arg),
            _ => return Err("Only one image file can be given".to_string()),
        }
    }

    path.map(|path| Options { path, ascii })
        .ok_or_else(|| "Image file is required".to_string())
}

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "prpic".to_string());
    let options = parse_options(args).unwrap_or_else(|error| {
        eprintln!("{error}");
        eprintln!("Usage: {program} [--ascii] <image_file>");
        std::process::exit(1);
    });

    let is_pdf = Path::new(&options.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));

    let loader: Box<dyn Loader> = if is_pdf {
        Box::new(PdfLoader { path: options.path })
    } else {
        Box::new(FileLoader { path: options.path })
    };

    let image = loader.load().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });

    let renderer: Box<dyn Renderer> = if options.ascii {
        Box::new(AsciiRenderer)
    } else {
        Box::new(HalfBlockRenderer)
    };

    let (terminal_width, terminal_height) = current_terminal_size();
    print!("{}", renderer.render(&image, terminal_width, terminal_height));
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
}
