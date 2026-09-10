# prpic

Prpic is a command-line tool that renders images and PDF pages directly in the terminal.

```bash
prpic photo.jpg
prpic --ascii document.pdf
prpic --text document.pdf
```

`--text` extracts a PDF's real text content instead of rasterizing it — useful since dense
body text easily becomes illegible once downsampled to terminal resolution.

Multi-page PDFs are shown one page at a time. When run in an interactive terminal, prpic
pauses between pages with a `-- MORE (n/total) --` prompt: press Enter to see the next page,
or `q` to stop early. When output is redirected or piped, all pages are printed back to back
with no prompts.

## Goals

- Render common image formats and every page of a PDF to the terminal, one page at a time
- Default to full 24-bit color output using half-block characters
- Support a plain `--ascii` fallback for terminals without color/Unicode support
- Ship as a single self-contained binary with no manual runtime setup, including PDF support

## Implementation

Prpic is written in Rust, split into a few small modules so new input formats and output
styles can be added independently:

- `Loader` (`src/loader.rs`) always produces pixel data (`RgbImage`) for a single image.
  `FileLoader` reads standard image formats via the `image` crate. This module also binds
  the bundled Pdfium library for PDF support.
- `Renderer` (`src/render.rs`) turns pixel data into terminal output. `HalfBlockRenderer`
  is the default; `AsciiRenderer` is used with `--ascii`.
- `PageSource` (`src/pager.rs`) is a content-agnostic abstraction for anything that can be
  rendered one page at a time (`page_count()` + `render_page(index)`). `run_paged()` drives
  any `PageSource` through the terminal, showing the `-- MORE --` prompt between pages.
  Because this trait doesn't know anything about images or PDFs, adding a new multi-page
  format later (e.g. a Word document backend) only requires a new `PageSource` implementation.
- `src/sources.rs` provides the concrete `PageSource` implementations: `ImagePageSource`
  (a single image, always one page), `PdfImagePageSource` (rasterizes each PDF page via
  Pdfium and a `Renderer`), and `PdfTextPageSource` (extracts each PDF page's text).

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
| [prpic_win_x64.zip](https://github.com/jalybeck/prpic/releases/latest/download/prpic_win_x64.zip) | Windows x64 | 2026-09-10 07:49 UTC |
| [prpic_linux_x64.tar.gz](https://github.com/jalybeck/prpic/releases/latest/download/prpic_linux_x64.tar.gz) | Linux x64 | 2026-09-10 07:49 UTC |
<!-- BUILD_TABLE_END -->
