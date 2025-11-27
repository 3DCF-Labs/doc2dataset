# 3DCF Configuration (`3dcf.toml`)

The CLI reads an optional `3dcf.toml` (or `--config path`) to supply sane defaults for every command. All fields are optional—the CLI flags always win.

```toml
[defaults.encode]
preset = "reports"          # reports | slides | news | scans
budget = 256                 # max cells per document
hyphenation = "merge"       # merge | preserve
drop_footers = true
dedup_window = 2             # pages to look back for duplicate headers
table_column_tolerance = 32  # px spacing required to flag tables
enable_ocr = true            # allow OCR fallback when text layer is missing
force_ocr = false            # always OCR even when text is present
ocr_langs = ["eng", "spa"]
heading_boost = 1.2          # importance multiplier when a cell looks like a heading
number_boost = 1.0           # importance multiplier when the cell contains numbers
footer_penalty = 0.4         # importance multiplier applied to footer-looking cells
early_line_bonus = 1.0       # weight applied to the first few lines on a page
table_mode = "auto"          # auto | csv | dims for `.3dcf.txt`
preset_label = "reports"     # string embedded in `.3dcf.txt` headers
budget_label = "auto"        # string embedded in `.3dcf.txt` headers
strict_numguard = false      # fail encode/decode if numeric guards diverge
numguard_units = "configs/units.txt" # optional whitelist of allowed units

[defaults.bench]
preset = "reports"
tokenizer = "cl100k_base"
budget = 256

[defaults.stats]
tokenizer = "cl100k_base"
```

Usage:

```bash
3dcf encode doc.pdf --config configs/infra.toml
3dcf bench datasets/financial
```

Tips:
- Keep corpora-specific knobs (e.g., `dedup_window`, OCR languages) in separate config files.
- Add the file to `.gitignore` if it contains private dataset paths.
- Combine with environment variables such as `RUST_LOG=debug` for verbose tracing.

## `doc2dataset.yaml`

`doc2dataset run --config doc2dataset.yaml` drives ingest → tasks → exports.

```
dataset_root: ./datasets/company
sources:
  - path: ./docs/policies
    pattern: "*.pdf"
  - path: ./docs/wiki_export
    pattern: "*.md,*.html"
tasks: [qa, summary]
ingest:
  preset: reports
  enable_ocr: true
  force_ocr: false
  ocr_langs: ["eng", "deu"]
exports:
  hf: true
  llama_factory:
    format: sharegpt
  openai: true
  axolotl:
    mode: chat
  rag_jsonl: true
```

Each source is ingested sequentially into the same `dataset_root`, so you can mix PDFs, Markdown exports, and other folders. The `ingest` block mirrors the CLI flags (`--preset`, `--enable-ocr`, etc.). Exports can be enabled independently—set `rag_jsonl: true` to materialize `exports/rag/train.jsonl` alongside HF/LLaMA/OpenAI/Axolotl files.

### Mixed formats (JSON + CSV)

The conversion layer lets you drop structured dumps directly into a `doc2dataset` run. Point the config at a directory and include the right globs:

```
dataset_root: ./datasets/dw
sources:
  - path: ./data
    pattern: "*.json,*.csv,*.csv.gz,*.xml,*.ini,*.toml,*.log"
tasks: [qa]
ingest:
  preset: reports
  enable_ocr: false
exports:
  hf: true
```

JSON becomes nested Markdown (headings + tables) and CSV turns into chunked Markdown tables before ingest, so the downstream QA/Summary/exports stack behaves like a PDF input. XML/RSS/Atom, gzip’d CSV/TSV, TeX/Bib/Bbl, INI/CFG/CONF, TOML, `.log`, and `.rtf` files follow the same path—just extend the glob pattern like above.
