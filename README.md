# prpic

Prpic is a command-line tool that renders images and PDF pages directly in the terminal.

```bash
prpic photo.jpg
prpic --ascii document.pdf
```

## Goals

- Render common image formats and PDF pages (first page) to the terminal
- Default to full 24-bit color output using half-block characters
- Support a plain `--ascii` fallback for terminals without color/Unicode support
- Ship as a single self-contained binary with no manual runtime setup, including PDF support

## Implementation

Prpic is written in Rust. Loading and rendering are split behind two traits so new
input formats and output styles can be added independently:

- `Loader` (`src/loader.rs`) always produces pixel data (`RgbImage`). `FileLoader` reads
  standard image formats via the `image` crate; `PdfLoader` rasterizes a PDF page via a
  bundled Pdfium library.
- `Renderer` (`src/render.rs`) turns pixel data into terminal output. `HalfBlockRenderer`
  is the default; `AsciiRenderer` is used with `--ascii`.

The Pdfium library is downloaded by `build.rs` from a pinned
[pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) release (checksum-verified),
cached locally, and placed next to the compiled binary for Windows and Linux. `PdfLoader`
loads it from the executable's own directory at runtime. Release archives ship the matching
library file and its license texts alongside the binary, so PDF support works without any
manual download or installation step.

## Status

Under development

## Downloads

Latest automatically built binaries. The table updates whenever a new git tag triggers a
build and publishes release packages; older versions are not kept.

<!-- BUILD_TABLE_START -->
| Package | Platform | Built |
| --- | --- | --- |
<!-- BUILD_TABLE_END -->
