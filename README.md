# prpic

Prpic is a command-line tool that renders images and PDF pages directly in the terminal.

```bash
prpic photo.jpg
prpic --ascii document.pdf
prpic --text document.pdf
prpic document.docx
prpic --text document.docx
prpic notes.txt
prpic document.md
prpic --text document.md
```

`--text` extracts a PDF's real text content instead of rasterizing it — useful since dense
body text easily becomes illegible once downsampled to terminal resolution.

.docx (Word) files are always shown as text — there's no pure-Rust engine available to
rasterize a docx's layout into pixels the way pdfium does for PDFs. The default rendering
keeps bold/italic/underline formatting via ANSI codes; `--text` strips that styling down to
plain text. Only modern `.docx` files are supported (not the legacy binary `.doc` format).
Since a `.docx` has no fixed page layout in the file itself, prpic splits it at explicit page
breaks (Ctrl+Enter in Word) where present, and additionally chunks the text to fit the
current terminal height so long documents without any explicit break still page correctly.

`.md`/`.markdown` files are rendered similarly: headings, bold/italic emphasis, lists,
blockquotes and code (inline and fenced blocks) are turned into ANSI-styled text by default,
or `--text` for plain text. Since markdown has no page concept at all, the whole file is
chunked purely by terminal height. Plain `.txt` files are just printed as-is, chunked by
terminal height the same way; there's no formatting to strip, so `--text` isn't relevant
for them.

Multi-page PDFs, docx, and markdown files are shown one page at a time. When run in an
interactive terminal, prpic pauses between pages with a `-- MORE (n/total) --` prompt: press
Enter to see the next page, or `q` to stop early. When output is redirected or piped, all
pages are printed back to back with no prompts.

## Goals

- Render common image formats and every page of a PDF, docx, or markdown file to the
  terminal, one page at a time
- Default to full 24-bit color output using half-block characters for images/PDFs
- Support a plain `--ascii` fallback for terminals without color/Unicode support
- Ship as a single self-contained binary with no manual runtime setup, including PDF support

## Implementation

Prpic is written in Rust, split into a few small modules so new input formats and output
styles can be added independently:

- `Loader` (`src/loader.rs`) always produces pixel data (`RgbImage`) for a single image.
  `FileLoader` reads standard image formats via the `image` crate. This module also binds
  the bundled Pdfium library for PDF support, and hosts `loader::run()` — the single entry
  point `main` calls into, which resolves a path's extension and flags to the right
  `PageSource` and renderer so `main` itself stays free of format-specific details.
- `Renderer` (`src/render.rs`) turns pixel data into terminal output. `HalfBlockRenderer`
  is the default; `AsciiRenderer` is used with `--ascii`. `TextRenderer` is the equivalent
  for extracted document text (`TextSpan`s, tagged with bold/italic/underline/code):
  `StyledTextRenderer` keeps that formatting as ANSI codes, `PlainTextRenderer` strips it for
  `--text` mode. `render.rs` also has `paginate_spans_by_terminal_height`, a shared helper
  that chunks any `Vec<TextSpan>` pages to the current terminal height, used by every
  text-based `PageSource`.
- `PageSource` (`src/pager.rs`) is a content-agnostic abstraction for anything that can be
  rendered one page at a time (`page_count()` + `render_page(index)`). `run_paged()` drives
  any `PageSource` through the terminal, showing the `-- MORE --` prompt between pages.
  Because this trait doesn't know anything about images, PDFs, or docx files, adding another
  multi-page format later only requires a new `PageSource` implementation.
- `src/sources.rs` provides the concrete `PageSource` implementations: `ImagePageSource`
  (a single image, always one page), `PdfImagePageSource` (rasterizes each PDF page via
  Pdfium and a `Renderer`), and `PdfTextPageSource` (extracts each PDF page's text).
- `src/docx_source.rs` provides `DocxPageSource` for `.docx` files, using the
  [docx-rs](https://github.com/bokuweb/docx-rs) crate to parse the document into pages of
  `TextSpan`s, splitting at explicit page breaks and further chunking by terminal height.
  It takes a `TextRenderer` just like `ImagePageSource` takes a `Renderer`, so the same
  struct serves both the default styled output and `--text` mode.
- `src/markdown_source.rs` provides `MarkdownPageSource` for `.md`/`.markdown` files, using
  the [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) crate to turn
  headings, emphasis, lists, blockquotes and code into `TextSpan`s. Like `DocxPageSource`, it
  takes a `TextRenderer` so it serves both styled and `--text` output.
- `src/text_source.rs` provides `TextFilePageSource` for plain `.txt` files. There's no
  formatting to preserve or strip, so unlike the docx/markdown sources it always renders
  through `PlainTextRenderer` directly rather than taking a `TextRenderer` choice.

The Pdfium library is downloaded by `build.rs` from a pinned
[pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) release (checksum-verified),
cached locally, and placed next to the compiled binary for Windows and Linux. It's loaded
from the executable's own directory at runtime. Release archives ship the matching library
file and its license texts alongside the binary, so PDF support works without any manual
download or installation step.

## Status

Under development

## Downloads

Latest automatically built binaries. The table updates whenever a new git tag triggers a
build and publishes release packages; older versions are not kept.

<!-- BUILD_TABLE_START -->
| Package | Platform | Built |
| --- | --- | --- |
| [prpic_win_x64.zip](https://github.com/jalybeck/prpic/releases/latest/download/prpic_win_x64.zip) | Windows x64 | 2026-09-10 11:44 UTC |
| [prpic_linux_x64.tar.gz](https://github.com/jalybeck/prpic/releases/latest/download/prpic_linux_x64.tar.gz) | Linux x64 | 2026-09-10 11:44 UTC |
<!-- BUILD_TABLE_END -->
