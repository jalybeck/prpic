mod docx_source;
mod loader;
mod pager;
mod render;
mod sources;

use std::env;

#[derive(Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) path: String,
    pub(crate) ascii: bool,
    pub(crate) text: bool,
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

    if let Err(error) = loader::run(&options) {
        eprintln!("{error}");
        std::process::exit(1);
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
