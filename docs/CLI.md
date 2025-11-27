# 3DCF CLI Guide

Every command honours the optional `3dcf.toml` config (see `docs/CONFIG.md`). Flags always override config values.

```
3dcf encode <input>
    [--preset reports|slides|news|scans]
    [--budget N] [--drop-footers]
    [--dedup-window PAGES]
    [--hyphenation merge|preserve]
    [--table-column-tolerance PX]
    [--heading-boost F] [--number-boost F]
    [--footer-penalty F] [--early-line-bonus F]
    [--table-mode auto|csv|dims]
    [--preset-label reports] [--budget-label auto]
    [--strict-numguard] [--numguard-units units.txt]
    [--enable-ocr] [--force-ocr] [--ocr-langs eng,spa]
    [--out tokens.3dcf] [--json-out tokens.3dcf.json] [--text-out context.txt]
    [--cells-out cells.jsonl]

3dcf decode <tokens.3dcf> [--text-out roundtrip.txt] [--json-out tokens.json] \
           [--page Z] [--select "z=3,x=120..240,y=40..100"] \
           [--strict-numguard] [--numguard-units units.txt]
3dcf serialize <tokens.3dcf> --out context.txt [--preview 80] [--table-mode auto|csv|dims] \
           [--preset-label reports] [--budget-label auto]
3dcf stats <tokens.3dcf> [--tokenizer cl100k_base|gpt2|o200k|anthropic|custom] \
           [--tokenizer-file path/to/tokenizer.json]
3dcf bench <datasets> [--mode encode|decode|full] [--budgets 64,128,auto] \
           [--preset ...] [--tokenizer ...] [--tokenizer-file ...] \
           [--gold /path/to/gold] [--output results.jsonl] \
           [--cer-threshold 0.02] [--wer-threshold 0.03] \
           [--numguard-max 0] [--encode-p95-max 500] [--decode-p95-max 500]
3dcf chunk <tokens.3dcf> <chunks.jsonl> [--cells 200] [--overlap 20] \
            [--mode cells|tokens|headings|table-rows] [--max-tokens 512] [--overlap-tokens 64]
3dcf embed <chunks.jsonl> <embeddings.jsonl> [--backend hash|openai] \
            [--dimensions 64] [--seed 1337] [--limit 1000] [--cache cache.json] \
            [--max-concurrency 4] [--retry-limit 5] [--retry-base-ms 500] \
            [--openai-model text-embedding-3-small] [--openai-api-key sk-...] \
            [--openai-base-url https://api.openai.com/v1] \
            [--cohere-model embed-multilingual-v3.0] [--cohere-api-key xxx] \
            [--cohere-base-url https://api.cohere.com/v1]
3dcf index <embeddings.jsonl> <index.bin>
3dcf search [--embeddings embeddings.jsonl | --index index.bin] "query text" \
            [--backend hash|openai] [--top-k 5] [--dimensions 64] [--seed 1337] \
            [--openai-model ...] [--openai-api-key ...] [--openai-base-url ...] \
            [--cohere-model ...] [--cohere-api-key ...] [--cohere-base-url ...]
3dcf qdrant-push --embeddings chunks.jsonl --url http://localhost:6333 --collection rag \
            [--api-key ...] [--batch 128] [--wait]
3dcf qdrant-search --embeddings chunks.jsonl --url http://localhost:6333 --collection rag \
            "query text" [--top-k 5] [--api-key ...] [--backend override]
3dcf report <results.jsonl> --out bench/report.html
3dcf encrypt <input> --out tokens.age --recipient AGE-...  # age recipient string
3dcf decrypt <tokens.age> --out tokens.3dcf --identity age.key
3dcf synth <out_dir> [--count 10] [--seed 42]
```

### Typical workflow

1. `3dcf encode report.pdf --preset reports --drop-footers --dedup-window 2 --out report.3dcf`
2. `3dcf decode report.3dcf --text-out roundtrip.txt` (sanity check: CLI will warn if NumGuards disagree)
3. `3dcf serialize report.3dcf --out context.txt --preview 96` (prompt payload with table sketches)
4. `3dcf stats report.3dcf --tokenizer cl100k_base`
5. `3dcf bench datasets/financial --preset reports --tokenizer cl100k_base --output bench/results.jsonl`
6. `3dcf report bench/results.jsonl --out bench/report.html`
7. `3dcf synth datasets/synthetic --count 25` (quick placeholder corpus)

Use `--page` to dump a specific page from a `.3dcf`, or `--select` to decode a rectangular region.
Selectors take the form `z=<page>,x=<start..end>,y=<start..end>`; single values (e.g., `x=120`)
lock to that coordinate.

When benchmarking with gold references, point `--gold` at a directory mirroring the dataset layout
and containing `.txt` files; `3dcf bench` will emit CER/WER and numeric-integrity stats alongside
token savings. (See `dataTests/gold/custom/sample_txt.3dcf.txt` for a miniature example that aligns
with `dataTests/custom/sample_txt.3dcf`.) Combine `--mode encode|decode|full` with
`--budgets 64,128,auto` to sweep multiple
macro-token targets in one run. Use `--tokenizer-file` when selecting `custom` to load a JSON spec
with `pat_str`, `mergeable_ranks`, and `special_tokens` fields so savings reflect downstream
tokenizers.

Encoder/serializer knobs:
- `--heading-boost`, `--number-boost`, `--footer-penalty`, `--early-line-bonus` adjust how the
  encoder ranks content when budgets force it to drop low-importance cells.
- `--table-mode auto|csv|dims` controls how table previews render inside `.3dcf.txt` (small tables
  can emit inline CSV snippets while large ones fall back to `rows/cols` summaries). The flag is
  available on both `encode` (when `--text-out` is set) and `serialize`.
- `--strict-numguard` makes the CLI fail if any numeric guard hash mismatches (or if a guard uses a
  unit not listed in `--numguard-units`). Without it, the CLI just prints warnings.
- `--cells-out` dumps per-cell metadata (coordinates, importance, preview) as JSONL so you can build
  curriculum-learning schedules or sampling manifests before training.

Encryption:
- `3dcf encrypt tokens.3dcf --out tokens.age --recipient AGE-...` uses age’s X25519 recipients. Use
  `age-keygen -o age.key` to create a keypair, then share the `age ...` public string with senders.
- `3dcf decrypt tokens.age --out tokens.3dcf --identity age.key` decrypts locally using the secret
  key. Only recipients encrypted with `--recipient` can decrypt the payload.

Chunking, embeddings, and vector search:
- `3dcf chunk tokens.3dcf chunks.jsonl --mode tokens --max-tokens 384 --overlap-tokens 96` walks
  the deterministic macro cells and emits token-bounded slices (also available: `--mode cells`,
  `--mode headings`, `--mode table-rows`). Each chunk gets a stable `chunk_id` plus metadata (page
  span, cell range, token count, dominant cell type, importance). The JSONL begins with a
  `chunk_meta` header describing the doc id, mode, window sizes, and chunker version.
- `3dcf embed chunks.jsonl embeddings.jsonl --backend hash` converts each chunk into a vector. The
  CLI prepends an `embed_meta` header (backend, model, vector length, seed, normalized flag).
  Re-running with `--cache cache.json` persists embeddings per backend/model namespace and hashes
  each chunk’s text so stale entries are invalidated. Use `--max-concurrency` and
  `--retry-limit/--retry-base-ms` to control HTTP worker count and exponential backoff. Providers
  supported today: deterministic hash, OpenAI (`--backend openai --openai-model ...`), and Cohere
  (`--backend cohere --cohere-model embed-multilingual-v3.0`). All vectors are L2-normalized for
  cosine search automatically.
- `3dcf index embeddings.jsonl index.bin` writes a binary “flat” store (bincode) that preserves the
  metadata plus every embedding. `3dcf search --embeddings ... "query"` performs brute-force cosine
  search directly from JSONL, whereas `--index index.bin` loads the binary store instead. Searches
  automatically reuse the embedding backend recorded in `embed_meta` unless you override it.
- `3dcf qdrant-push --embeddings ...` streams vectors into a Qdrant collection (local or managed).
  The CLI ensures the collection exists (correct dimensionality), batches upserts, and copies doc /
  chunk payloads so Qdrant can return previews. Add `--api-key` if your cluster enforces auth.
- `3dcf qdrant-search --embeddings ... --url ...` embeds a query with the same backend recorded in
  the JSONL/index, runs Qdrant’s ANN search, and prints the scored payloads. Use this when you want
  a persistent ANN service instead of the built-in brute-force search.

## doc2dataset CLI

Turn unstructured docs into finetuning datasets:

- `doc2dataset ingest <input> --output ./datasets/foo --pattern "*.pdf" --preset reports --enable-ocr --ocr-langs eng,spa`
- `doc2dataset tasks ./datasets/foo --tasks qa,summary`
- `doc2dataset export hf|llama-factory|openai|axolotl|rag-jsonl ./datasets/foo [--format alpaca|sharegpt] [--mode chat|text]`
- `doc2dataset quickstart ./docs`
- `doc2dataset run --config doc2dataset.yaml`

Exports produce:

- Hugging Face: `exports/hf/train.jsonl` and `exports/hf/train_chat.jsonl`
- LLaMA-Factory: `exports/llama_factory/alpaca.jsonl` or `sharegpt.jsonl` (`--format sharegpt`)
- OpenAI / Axolotl: `exports/openai/finetune.jsonl`, `exports/axolotl/{chat,text}.jsonl`
- Retrieval-aware: `doc2dataset export rag-jsonl` → `exports/rag/train.jsonl`

For quick experimentation: encode a doc, run `chunk` → `embed` → `search` to obtain retrieval-ready
snippets, or add `index` in between for faster repeated queries. The metadata headers make it easy
to pipe these files into custom ANN stores (FAISS, Lance, pgvector, or Qdrant) while keeping backend
info attached to every artifact.

All commands emit `tracing` logs (set `RUST_LOG=debug` for verbose diagnostics).
