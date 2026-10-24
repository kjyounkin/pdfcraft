# PdfCraft: unofficial build with stylus mode

This is an unofficial, modified version of [PdfCraft](https://github.com/storytold/pdfcraft), the
open-source PDF reader and editor written in Rust. It is based on PdfCraft by the ArtCraft team, and
it is not made, endorsed or supported by them. Report problems with this build in
[this repository's issues](https://github.com/kjyounkin/pdfcraft/issues), not upstream.

## What this build adds

**Stylus mode for pen tablets in the browser** (branch `stylus-mode`):

- The pen uses the chosen tool: highlight, ink, select and the rest.
- A finger drag moves the page instead of marking it. Toolbar taps and pinch zoom work as usual.
- A palm resting on the screen while the pen writes is ignored.
- It turns on the first time a pen is used and is remembered. Add `?stylus=off` to the address to
  keep it off, or `?stylus=on` to start with it on.

The change is in `crates/ui-egui/src/stylus.rs`, with small hooks in `crates/ui-egui/src/canvas.rs`
and `apps/pdfcraft-web/src/main.rs`. The `stylus-mode` branch carries only that change, so it can
be proposed upstream; `unofficial-build` adds the removal of upstream's branding.

## Changes from upstream

- Stylus mode (above).
- Upstream's ArtCraft name, wordmark and logos are removed, as upstream's brand terms require of
  modified versions. The app's links point to this repository and to upstream PdfCraft.

Everything else, including the documentation, is upstream's: see the
[upstream README](https://github.com/storytold/pdfcraft#readme).

## Building the web version

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd apps/pdfcraft-web && trunk build --release   # the site is written to dist/web
```

Hosting notes are in `packaging/web/README.md`.

## License

PdfCraft is licensed under either of the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option, and so are the changes in this
fork. Copyright (c) 2026 ArtCraft Team and the PdfCraft contributors. Required notices are in
[NOTICE](NOTICE), and third-party material is listed in [ATTRIBUTION.md](ATTRIBUTION.md).
