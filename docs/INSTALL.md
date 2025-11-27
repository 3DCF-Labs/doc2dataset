# Installing Native Dependencies

3DCF uses optional native backends for higher-fidelity ingestion. Build-time features are toggled
through `--features pdfium` and/or `--features ocr`. The CLI binary prints the active feature set via
`3dcf --version`.

## macOS

```bash
brew install tesseract tesseract-lang leptonica
```

PDFium does not currently ship as a Homebrew formula. Download the latest
`pdfium-mac-arm64.tgz` (or x64) from
<https://github.com/bblanchon/pdfium-binaries/releases>, unpack it somewhere
stable, and point the build at it:

```bash
mkdir -p ~/opt/pdfium
curl -L https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-mac-arm64.tgz \
  | tar -xz -C ~/opt/pdfium

export PDFIUM_LIB_DIR=~/opt/pdfium/lib
export PDFIUM_INCLUDE_DIR=~/opt/pdfium/include
```

`leptess` expects the Leptonica shared library to be named `liblept`. When
building on Apple Silicon, Homebrew installs it as `libleptonica`. Add a
symlink once after install so the linker finds it automatically:

```bash
ln -sf /opt/homebrew/opt/leptonica/lib/libleptonica.dylib /opt/homebrew/opt/leptonica/lib/liblept.dylib
ln -sf /opt/homebrew/opt/leptonica/lib/libleptonica.a /opt/homebrew/opt/leptonica/lib/liblept.a
```

If Homebrew installed to a custom prefix, export `PDFIUM_LIB_DIR` / `PDFIUM_INCLUDE_DIR`
accordingly.

## Linux (Debian/Ubuntu)

```bash
sudo apt-get update
sudo apt-get install -y libtesseract-dev tesseract-ocr libclang-dev
# PDFium: use the upstream binary tarball
curl -L https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz \
  | sudo tar xz -C /usr/local
```

Set `PDFIUM_LIB_DIR=/usr/local/lib` and `PDFIUM_INCLUDE_DIR=/usr/local/include` before running
`cargo build -F pdfium`. Some distributions package Leptonica as `libleptonica`; if your linker
complains about `-llept`, add a compatibility symlink or pass
`RUSTFLAGS="-L native=/usr/lib/x86_64-linux-gnu"` (update the path for your distro).

## Windows (MSVC)

1. Install [Tesseract OCR](https://github.com/UB-Mannheim/tesseract/wiki) and add it to `PATH`.
2. Download the latest pdfium binary from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)
   and extract `pdfium.dll` / headers somewhere stable.
3. Provide `PDFIUM_LIB_DIR`, `PDFIUM_INCLUDE_DIR`, and `VCPKGRS_DYNAMIC=1` so `bindgen` can locate
   the SDK when compiling the core crate with `--features pdfium`.

## Feature matrix

| Feature  | Cargo flag | Native deps | Notes |
|----------|------------|-------------|-------|
| PDFium   | `pdfium`   | pdfium SDK  | Enables high-quality text layer extraction for complex PDFs. |
| OCR      | `ocr`      | Tesseract   | Falls back to OCR when the PDF has images only. |

You can build lean binaries (no native deps) via `cargo build -p three_dcf_cli` or ship multiple
profiles (cpu/pdfium/ocr/full) as part of your release pipeline.
