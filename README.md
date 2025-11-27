# 3DCF / doc2dataset

Open document layer and doc→dataset pipeline for LLMs, with NumGuard numeric integrity and multi-framework exports.

3DCF/doc2dataset ingests PDFs, Markdown, plain text and other text-like formats into a normalized index (`documents.jsonl`, `pages.jsonl`, `cells.jsonl`), extracts NumGuard hashes for numeric cells, and generates QA/Summary/RAG datasets plus exports for HuggingFace, LLaMA-Factory, Axolotl, OpenAI, and custom RAG stacks. The workspace bundles the Rust core, CLI, doc2dataset pipeline, HTTP service + UI, and Python/Node bindings.

## Documentation

- [Technical Report / Spec](./3dcf_doc2dataset_spec.md)
- [CLI guide](./docs/CLI.md)
- [Configuration guide](./docs/CONFIG.md)
- [Data format reference](./docs/FORMAT.md)
- [Installation notes](./docs/INSTALL.md)
- **Evaluation data (GitHub Releases)** – evaluation corpora and metrics are distributed as a GitHub Release asset (see the latest release for a `3dcf-eval-*.tar.*` archive). Download the archive, unpack it at the repo root (it recreates the `eval/` tree), and then follow `eval/README.md`.

## Features

- **Document layer standard** – deterministic macro-cells with `kind` / `bbox` / `importance` stored in three JSONL files: `documents.jsonl`, `pages.jsonl`, and `cells.jsonl`.
- **NumGuard numeric integrity** – per-cell number hashes with A/B/C/D coverage; in our evaluation, all A-bucket corruptions are detected (recall 1.0).
- **Token-efficient contexts** – macro-cell contexts are typically 3–6× smaller in tokens than naive pdfminer/Unstructured baselines on our micro-corpora, while maintaining or improving QA accuracy and numeric faithfulness (see Technical Report).
- **doc→dataset tasks** – reusable `qa.jsonl`, `summary.jsonl`, and `rag.jsonl` samples with metrics for observability.
- **Multi-framework exports** – ready-to-use datasets for HuggingFace (text/chat), LLaMA-Factory (Alpaca/ShareGPT), Axolotl (text/chat), OpenAI finetune (`messages` JSONL), and a generic RAG JSONL.
- **Rust-native core + bindings** – CLI (`three_dcf_cli`) and HTTP service (`three_dcf_service`), plus Python (`three_dcf_py`) and Node (`three_dcf_node`) bindings for easy integration.

## Who is this for?

- **ML / AI platform teams** – need a reproducible document layer that feeds RAG and fine-tuning pipelines.
- **Fintech / regulatory / analytics teams** – care about numeric correctness in reports, filings, and policies (NumGuard).
- **LLM researchers / OSS devs** – want an open, inspectable doc→dataset standard with a realistic evaluation suite.

## Quickstart

### 1. Build the CLI

```bash
git clone https://github.com/<org>/3dcf.git
cd 3dcf

# Build and run the main CLI without installing
cargo run -p three_dcf_cli -- --help
```

### 2. Run doc2dataset on an example config

```bash
# Run the doc2dataset pipeline directly via Cargo
cargo run -p doc2dataset -- run   --config examples/doc2dataset/openai-finetune/doc2dataset.yaml

# Inspect the generated dataset
tree datasets/default -L 2
```

You should see `datasets/default/{index,raw/3dcf,samples,exports}` populated with QA/Summary/RAG samples and finetune exports.

### 3. Minimal macro-cell smoke test

```bash
# Encode a sample Markdown report
cargo run -p three_dcf_cli -- encode   datasets/sample/sample_report.md   --preset reports   --budget 256   --out sample.3dcf   --text-out sample.3dcf.txt   --json-out sample.3dcf.json

# Serialize context and compute token stats
cargo run -p three_dcf_cli -- serialize sample.3dcf   --out sample.context.txt   --preview 96

cargo run -p three_dcf_cli -- stats sample.3dcf   --tokenizer cl100k_base
```

If you prefer installing binaries instead of `cargo run`, you can do:

```bash
cargo install --path crates/cli --force       # installs `three_dcf_cli` as `3dcf` on $PATH
cargo install --path crates/doc2dataset --force

3dcf --help
doc2dataset --help
```

## Layout

```text
3dcf/
  Cargo.toml                # workspace definition
  proto/3dcf.proto          # Protobuf schema
  crates/
    core/                   # encode/decode/serializer/stats/NumGuard
    cli/                    # CLI (`three_dcf_cli` → `3dcf`)
    doc2dataset/            # doc→dataset pipeline (`doc2dataset`)
    service/                # HTTP service + UI
    index/, llm/, rag/      # index/LLM/RAG helpers
    ffi-node/, ffi-py/      # Node/Python bindings
  datasets/                 # sample corpora + README
  docs/                     # CLI/config/format guides
  eval/                     # local evaluation runs (downloaded from GitHub Releases; not tracked in git)
  examples/, recipes/       # integration examples and recipes
```

> Note: `eval/` is intended to be populated from a GitHub Release archive (e.g., `3dcf-eval-v0.1.tar.gz`) and should be added to your `.gitignore` to avoid committing large binary artifacts.

## doc2dataset in practice

`doc2dataset` is a separate CLI that orchestrates ingest, task generation, and exports based on a YAML config.

Environment variables:

- `DOC2DATASET_PROVIDER` – LLM provider (`openai`, `anthropic`, `local`, etc.).
- `DOC2DATASET_MODEL` – model name (`gpt-4.1-mini`, `claude-3.5-sonnet`, ...).
- `DOC2DATASET_LANG` – language code (e.g., `en`).

Ingest options (in configs or flags) mirror the core encoder:

- `preset` – encoder preset (`reports`, `news`, etc.).
- `enable_ocr` / `force_ocr` – control OCR usage.
- `ocr_langs` – Tesseract language codes (e.g., `["eng"]`).

### Supported formats and automatic conversions

`FileFormat::from_path` in `crates/doc2dataset/src/model.rs` normalizes file extensions to a small enum, and `convert::prepare_document` either passes the original file to 3DCF or converts it to temporary Markdown before ingest.

Currently supported conversions (see `crates/doc2dataset/src/convert/`):

- **HTML / XML** – `*.html`, `*.htm`, `*.xml`, `*.xhtml`, `*.rss`, `*.atom`  
  → converted to Markdown via a simple HTML-to-text pass (`convert/html.rs`).

- **JSON / YAML / TOML / INI** – `*.json`, `*.yaml`, `*.yml`, `*.toml`, `*.ini`, `*.cfg`, `*.conf`  
  → parsed into a normalized JSON structure and rendered as nested headings + key/value sections; simple arrays of objects are rendered as Markdown tables when keys align (`convert/structured.rs`).

- **CSV / TSV / compressed variants** – `*.csv`, `*.tsv`, `*.csv.gz`, `*.tsv.gz`  
  → parsed with the `csv` crate and emitted as Markdown tables, chunked at 50 rows per table by default (`convert/tabular.rs`).

- **TeX / Bib / Bbl** – `*.tex`, `*.bib`, `*.bbl`  
  → flattened into headings and text; `tabular` blocks are rendered as Markdown tables (`convert/tex.rs`, `convert/bib.rs`).

- **Logs / RTF** – `*.log`, `*.rtf`  
  → read as UTF-8 and wrapped as simple text blocks with a top-level heading based on the file stem (`convert/log.rs`, `convert/rtf.rs`).

- **PDF / Markdown / plain text** – `*.pdf`, `*.md`, `*.markdown`, `*.txt`  
  → passed directly to 3DCF core ingest.

- **Images** – `*.png`, `*.jpg`, `*.jpeg`, `*.gif`, `*.tif`, `*.tiff`, `*.bmp`, `*.webp`  
  → treated as `FileFormat::Image` and passed to core ingest; OCR is applied if the preset and flags enable OCR (see `three_dcf_core::ocr`).

Unsupported or unknown extensions are ingested as-is (if possible) or skipped with a log entry.

### Example `doc2dataset.yaml`

```yaml
dataset_root: ./datasets/company

sources:
  - path: ./docs/policies
    pattern: "*.pdf"
  - path: ./docs/wiki_export
    pattern: "*.md,*.html,*.json,*.csv"

tasks: [qa, summary]

exports:
  hf: true
  llama_factory:
    format: sharegpt
  openai: true
  axolotl:
    mode: chat
  rag_jsonl: true

ingest:
  preset: reports
  enable_ocr: false
  ocr_langs: ["eng"]
```

Running:

```bash
doc2dataset run --config doc2dataset.yaml
```

(or `cargo run -p doc2dataset -- run --config doc2dataset.yaml`) will:

- ingest all matching files,
- build a 3DCF index under `datasets/company/index/`,
- generate `samples/qa.jsonl` and `samples/summary.jsonl`,
- emit the selected exports under `datasets/company/exports/`.

## Context + ask commands (CLI)

The `three_dcf_cli` binary exposes helper commands for manual experiments:

```bash
# Build a compressed context from a PDF
cargo run -p three_dcf_cli -- context input.pdf   --preset reports   --budget 256   --tokenizer cl100k_base

# Ask different providers with the same compressed context
cargo run -p three_dcf_cli -- ask-openai    input.pdf --preset reports --budget 256 --model gpt-4.1-mini
cargo run -p three_dcf_cli -- ask-anthropic input.pdf --preset reports --budget 256 --model claude-3-5-sonnet
cargo run -p three_dcf_cli -- ask-gemini    input.pdf --preset reports --budget 256 --model gemini-1.5-flash
cargo run -p three_dcf_cli -- ask-deepseek  input.pdf --preset reports --budget 256 --model deepseek-chat
```

All of these subcommands share the same encoder options and metrics (tokens, savings, NumGuard coverage), and can be used in CI with `--quiet` to suppress summaries.

## HTTP service + Docker

The `three_dcf_service` crate exposes the core functionality over HTTP:

```bash
cargo run -p three_dcf_service
```

By default it starts an Axum server on `0.0.0.0:8000` with:

- `POST /encode` – multipart upload (PDF + options) returning encoded 3DCF documents and stats.
- `GET /` – a simple bundled UI that lets you upload documents from a browser and inspect contexts.

The root `Dockerfile` builds a static image with the same binary. You can run it with:

```bash
docker build -t three-dcf-service .
docker run -p 8000:8000 three-dcf-service
```

## Evaluation & observability

`3dcf bench` emits JSONL with CER/WER, numeric guard mismatches, throughput, and RSS per document. Feed it to `3dcf report` for an HTML dashboard, or into your monitoring stack (Prometheus/Grafana, etc.).

### Eval data from GitHub Releases

The canonical evaluation corpora and metrics are distributed via GitHub Releases:

1. Go to the **Releases** page of this repository.
2. Download the latest `3dcf-eval-*.tar.*` archive (for example, `3dcf-eval-v0.1.tar.gz`).
3. From the repo root, unpack it:

   ```bash
   tar -xzf 3dcf-eval-v0.1.tar.gz   # or the filename you downloaded
   ```

4. You should now have an `eval/` directory with `README.md`, raw documents, and JSONL metrics.

The `eval/` directory is intentionally **not committed to git**; add it to `.gitignore` and treat it as a local cache of evaluation runs.

## Examples & recipes

- `examples/doc2dataset/openai-finetune/` – OpenAI finetune JSONL export.
- `examples/doc2dataset/llama-factory-sharegpt/` – LLaMA-Factory ShareGPT config.
- `examples/doc2dataset/axolotl/` – Axolotl chat/text export.
- `recipes/3dcf-recipes/langchain-rag/` – LangChain loader/compressor/reader.
- `recipes/3dcf-recipes/openai-qa/` – Python helper that calls `three_dcf_py` then OpenAI `/responses`.
- `recipes/3dcf-recipes/numguard-demo/` – NumGuard corruption demo.

## Why 3DCF vs. plain OCR/Markdown?

- **Deterministic containers** – every macro-cell carries hashes, coordinates, and NumGuard metadata, making it easy to diff, audit, and replay pipelines.
- **Token-aware pruning** – headings, tables, and numeric-heavy cells are prioritized to meet strict budgets without losing critical context.
- **Prompt-friendly previews** – `.3dcf.txt` mirrors layout with table sketches that RAG prompts can use directly.
- **Observability baked in** – `3dcf bench` + `3dcf report` track CER/WER, numeric guards, throughput, and memory.

## Testing

```bash
# Default tests
cargo test

# Full surface (PDFium + OCR, macOS example)
export PDFIUM_LIB_DIR=~/opt/pdfium/lib
export PDFIUM_INCLUDE_DIR=~/opt/pdfium/include
export RUSTFLAGS='-L native=/opt/homebrew/opt/leptonica/lib -L native=/opt/homebrew/lib'
cargo test --all --all-features
```

## Bindings

- **Node.js** (`crates/ffi-node`): `npm install && npm run build` (or `cargo build -p three_dcf_node`).
- **Python** (`crates/ffi-py`): `maturin develop -m crates/ffi-py/Cargo.toml` (or `cargo build -p three_dcf_py`).

Both bindings expose `encode`, `decode_text`, `stats`, and related helpers using the same tokenizer names as the CLI (`cl100k_base`, `o200k`, `anthropic`).