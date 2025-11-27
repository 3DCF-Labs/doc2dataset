# Encode / Serialize / Stats entrypoints

## Crate map
- **Core processing** – `crates/core` is the `three_dcf_core` crate. It implements PDF/text ingestion (`encoder.rs`), the `Document` model (`document.rs`), serialization helpers (`serializer.rs`) and token/quality metrics (`stats.rs`, `metrics.rs`).
- **CLI** – `crates/cli` builds the `3dcf` binary via Clap (`clap::Parser` in `src/main.rs`). Subcommands are enumerated in `Commands` and dispatched through a single `match` in `main`.
- **Python bindings** – `crates/ffi-py` exposes the encoder/decoder/stats helpers via `pyo3`. Functions `encode`, `decode_text`, and `stats` wrap the core crate.
- **Node bindings** – `crates/ffi-node` provides the same surface area through `napi` (see exported `encode_file`, `decode_text`, `stats`).

## Encode pipeline touchpoints
- **CLI** – `Commands::Encode` in `crates/cli/src/main.rs` wires Clap args into an `Encoder::builder`. The builder configures preset/budget/filter knobs before calling `encoder.encode_path(&input)`.
- **Core entrypoint** – `Encoder::encode_path` (`crates/core/src/encoder.rs`) loads an `EncodeInput` from disk, expands to pages/lines, then forwards into `Encoder::encode` which produces a `Document` (adds pages, cells, numguards, applies dedup/budget, etc).
- **Document model** – `crates/core/src/document.rs` defines the persisted binary/JSON representation and helper methods (numguard, serialization, etc.).

## Serialize / text output
- **CLI** – `Commands::Serialize` reuses `TextSerializer` to write `.context.txt` style dumps; `Commands::Encode` optionally calls it when `--text-out` is provided.
- **Core serializer** – `crates/core/src/serializer.rs` exposes `TextSerializer::{write_textual,to_string}` with table rendering / preview controls.

## Stats / token estimator
- **CLI** – `Commands::Stats` loads a `.3dcf`/JSON doc and calls `Stats::measure` with the requested tokenizer.
- **Core** – `crates/core/src/stats.rs` houses:
  - `TokenizerKind` enum (cl100k, gpt2, anthropic, custom JSON) with `build()` returning a `tiktoken_rs::CoreBPE`.
  - `Stats::measure` which decodes the doc back to raw text, serializes to context text, tokenizes both, and reports `tokens_raw`, `tokens_3dcf`, `cells`, `unique_payloads`, `savings_ratio`.

## Existing metrics infrastructure
- There is no shared “encode metrics” struct today. NumGuard/accuracy metrics live in `crates/core/src/metrics.rs`, while encode command just prints success.
- Token estimator exists via `Stats` (CLI stats + ffi bindings) but is not yet integrated with encode/serialize flows.
- No tokenizer abstractions exist outside the stats module.

## What we’ll extend
- Introduce a reusable `Metrics` struct in `three_dcf_core` (likely `crates/core/src/metrics.rs` alongside NumGuard stats) capturing pages/lines/cells/dedup/token data.
- Update `Encoder::encode[_path]` to surface `(Document, Metrics)` so CLI/SDKs can report summary data after encode/context generation.
- Build CLI additions (`context`, `ask-*`) on top of these entrypoints, reusing `TextSerializer` and the tokenizer helpers from `stats.rs`.
- Python/Node bindings and forthcoming microservice/RAG examples will import the same encode helpers, so this document references their existing wrappers for future expansion.
