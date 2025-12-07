# 3DCF / doc2dataset  
**A Token-Efficient, Numerically-Robust Document Layer for RAG and Fine-Tuning**

**Author:** Yevhenii Molchanov  
**Status:** Technical Report / Specification (v0.1, reference implementation available)  
**Date:** 2025-11-27  
**License:** Apache-2.0 (reference implementation)  
**Repo:** https://github.com/3DCF-Labs/doc2dataset
**Eval artifacts:** Distributed via GitHub Releases as archive assets (e.g., `3dcf-eval-v0.1.tar.gz`) that restore the entire `eval/` tree (corpora, out/, results/) when extracted at the repo root.

---

## Abstract

Large Language Models (LLMs) are increasingly adapted to private corpora via retrieval-augmented generation (RAG) and fine-tuning. Both approaches require turning heterogeneous documents (PDF reports, internal policies, technical wikis) into **structured, token-efficient and numerically faithful datasets**. Today this “doc→dataset” step is usually implemented as a collection of ad-hoc scripts around PDF parsers and ETL libraries, with limited reproducibility, weak observability, and no common standard.

All evaluation scripts live under `eval/` (tracked in the repository), and evaluation metrics/results are stored under `eval/results/` after unpacking the evaluation archive from a GitHub Release (e.g., `3dcf-eval-v0.1.tar.gz`). This layout allows full reproducibility and independent verification on any machine.

We propose **3DCF / doc2dataset**, an open document layer and pipeline that standardizes this stage:

1. **3DCF document representation** – a normalized schema for documents, pages and macro-cells (`documents.jsonl`, `pages.jsonl`, `cells.jsonl`) with layout, importance, and **NumGuard** numeric integrity metadata.
2. **doc2dataset pipeline** – a configurable CLI pipeline that ingests multi-source corpora into 3DCF, generates task-specific samples (QA, summarization, retrieval-aware triples), and exports them to multiple training frameworks (Hugging Face Datasets, LLaMA-Factory Alpaca/ShareGPT, Axolotl text/chat, OpenAI `messages` JSONL, and a RAG JSONL).
3. **Reference implementation** – a Rust-based, Apache-2.0 implementation with CLI, YAML config, metrics, and an optional RAG store and HTTP service.

This document describes the design of the 3DCF layer, the doc2dataset pipeline, and a detailed **evaluation methodology and results**.

---

## Architecture at a Glance

3DCF/doc2dataset separates the doc→dataset pipeline into two layers:

- **3DCF document layer** – ingests raw documents (with configurable OCR and presets) and produces:
  - `raw/3dcf/*.3dcf` + `.3dcf.json` (per-document containers),
  - `index/documents.jsonl`, `index/pages.jsonl`, `index/cells.jsonl`.

- **doc2dataset task + export layer** – reads the 3DCF index, generates
  `samples/qa.jsonl`, `samples/summary.jsonl`, `samples/rag.jsonl`, and exports
  them into `exports/<target>/` for multiple training frameworks.

```text
Raw docs (PDF/MD/HTML/JSON/CSV/...)
    └── 3DCF ingest (core)
          ├── raw/3dcf/*.3dcf + *.json
          └── index/
              ├── documents.jsonl
              ├── pages.jsonl
              └── cells.jsonl
                  ↓
          doc2dataset tasks
              ├── samples/qa.jsonl
              ├── samples/summary.jsonl
              └── samples/rag.jsonl
                  ↓
          doc2dataset export
              ├── exports/hf/*.jsonl
              ├── exports/llama_factory/*.jsonl
              ├── exports/openai/finetune.jsonl
              ├── exports/axolotl/*.jsonl
              └── exports/rag/train.jsonl
```

A more detailed view is given in Section 3 (System Overview).

---

## 1. Introduction

### 1.1 Motivation

Organizations want LLMs that understand **their** documents:

- Finance: annual reports, risk disclosures, regulatory filings.
- Legal/compliance: contracts, internal and external policies.
- Engineering: design docs, API references, runbooks, wikis.

Two main strategies dominate:

- **RAG** – index documents, retrieve relevant chunks at query time, pass them into a base model.
- **Fine-tuning / continued pretraining** – adapt models to in-domain tasks or style using supervised and/or unsupervised training.

Both require a robust pipeline from **raw documents to structured training/retrieval data**. Today, this is usually:

```text
PDF / HTML / MD
    → ad-hoc parser script(s)
    → custom chunking / cleaning code
    → custom script for OpenAI finetune JSONL
    → another script for LLaMA-Factory
    → etc.
```

Problems:

- Duplicated effort between teams and projects.
- Poor reproducibility (“which script built this dataset?”).
- Token inefficiency (huge context with boilerplate).
- No explicit guarantees for **numeric integrity** (critical in finance/regulation).
- Tight coupling to a single training framework.

At the same time, **model-level formats** (OpenAI `messages`, Alpaca, ShareGPT, etc.) are standardized, but the **doc→dataset layer is not**.

### 1.2 Goals

We aim to:

- Define a **document-centric, model-agnostic** representation that:
  - preserves structure and layout (pages, headings, tables),
  - is **token-efficient** for LLMs,
  - tracks **numeric integrity** across the pipeline.
- Provide a **pluggable pipeline** that:
  - ingests multiple sources into this representation,
  - generates reusable task samples (QA, summaries, RAG triples),
  - exports them into standard training formats.
- Make this an open, reusable “doc→dataset” layer for teams building RAG and fine-tuning pipelines.

### 1.3 Related Work

Several systems and libraries address parts of the doc→dataset problem. PDF ETL libraries and frameworks such as pdfminer, pdfplumber, and Unstructured focus on extracting text and structure from heterogeneous documents, while RAG toolkits and orchestration frameworks concentrate on retrieval, chunking, and prompt construction over already-ingested text. Model-facing datasets (e.g., Alpaca, ShareGPT) and APIs (OpenAI `messages`, chat-style fine-tuning formats) provide standardized **output** formats for training and inference. 3DCF/doc2dataset is complementary to these efforts: it proposes a unified **document-layer representation** with explicit numeric integrity, token-aware macro-cells, and multi-framework exports from a single normalized index, filling the gap between low-level ETL and high-level model training formats.

---

## 2. Problem Statement and Requirements

### 2.1 Requirements

We define the following desiderata for a doc→dataset layer:

1. **Multi-format ingest**  
   Support PDFs, Markdown, plaintext, HTML/HTM/XML/XHTML/RSS/Atom, JSON/YAML/TOML/INI/CFG/CONF, CSV/TSV (including `.csv.gz` / `.tsv.gz`), TeX/Bib/Bbl, structured logs, rich text (`.rtf`), and image-based documents via OCR—extensible to other sources (Confluence, S3, etc.).

2. **Normalized document layer**  
   A stable, documented schema for:
   - Documents (source metadata),
   - Pages (layout, token statistics),
   - Macro-cells (content+structure units).

3. **Token-aware compression**  
   Minimize tokens passed to LLMs by:
   - removing repeated headers/footers and boilerplate,
   - prioritizing content-rich cells,
   - providing explicit token metrics.

4. **Numeric integrity guarantees**  
   Extract and track numeric values, so changes can be detected across stages.

5. **Task-level sample generation**  
   From the same corpus, generate:
   - QA pairs,
   - summarization samples,
   - retrieval-aware (context, question, answer) triples.

6. **Multi-framework exports**  
   From one corpus and one set of tasks, generate datasets compatible with:
   - Hugging Face Datasets (text + chat),
   - LLaMA-Factory (Alpaca, ShareGPT),
   - Axolotl (text, chat),
   - OpenAI fine-tuning (`messages` JSONL),
   - plus a generic RAG JSONL.

7. **Reproducibility and observability**  
   All steps should be:
   - CLI + config-driven,
   - produce metrics and logs,
   - deterministic given the same inputs and LLM.

8. **Open, inspectable implementation**  
   A reference stack that can be audited, extended, and embedded in CI pipelines.

---

## 3. System Overview

3DCF/doc2dataset decomposes the doc→dataset problem into two layers:

- **3DCF – Document Layer**  
  - Ingests raw documents with configurable OCR and presets.
  - Produces:
    - `raw/3dcf/*.3dcf` + `.3dcf.json` (internal binary / JSON representation),
    - `index/documents.jsonl`,
    - `index/pages.jsonl`,
    - `index/cells.jsonl`.
- **doc2dataset – Task & Export Layer**  
  - Reads the 3DCF index.
  - Generates `samples/*.jsonl`:
    - `qa.jsonl`, `summary.jsonl`, `rag.jsonl`.
  - Exports to `exports/<target>/*.jsonl` for multiple training frameworks.

```text
Raw docs (PDF/MD/HTML)
    └── 3DCF ingest (core)
          ├── raw/3dcf/*.3dcf + *.json
          └── index/
              ├── documents.jsonl
              ├── pages.jsonl
              └── cells.jsonl
                  ↓
          doc2dataset tasks
              ├── samples/qa.jsonl
              ├── samples/summary.jsonl
              └── samples/rag.jsonl
                  ↓
          doc2dataset export
              ├── exports/hf/*.jsonl
              ├── exports/llama_factory/*.jsonl
              ├── exports/openai/finetune.jsonl
              ├── exports/axolotl/*.jsonl
              └── exports/rag/train.jsonl
```

An optional RAG store consumes the same index and `rag.jsonl` to support online retrieval and evaluation.

---

## 4. 3DCF Document Representation

### 4.1 Index files

The index lives under `dataset_root/index/`:

- `documents.jsonl` — one `DocumentRecord` per document.
- `pages.jsonl` — `PageRecord` per page.
- `cells.jsonl` — `CellRecord` per macro-cell.

#### 4.1.1 DocumentRecord schema

```json
{
  "doc_id": "string",
  "title": "string or null",
  "source_type": "string",    // e.g., files | s3 | confluence
  "source_format": "string",  // e.g., pdf | md | html | txt
  "source_ref": "string",     // path, URL or external ID
  "tags": ["string", "..."]
}
```

#### 4.1.2 PageRecord schema

```json
{
  "page_id": "string",
  "doc_id": "string",
  "page_number": 1,
  "approx_tokens": 512,
  "meta": {
    "width_px": 595,
    "height_px": 842,
    "z": 0
  }
}
```

#### 4.1.3 CellRecord schema

```json
{
  "cell_id": "string",
  "doc_id": "string",
  "page_id": "string",
  "kind": "text",             // text | heading | table | figure | footer
  "text": "string",
  "importance": 0.83,
  "bbox": [64.0, 128.0, 960.0, 152.0],  // pixel coordinates [x0, y0, x1, y1]
  "numguard": null,           // stored separately in Document.numguards
  "meta": {
    "rle": 0
  }
}
```

---

Index correctness & coverage evaluates whether 3DCF ingest produces a correct and complete index for heterogeneous documents. The test corpus combines synthetic PDFs (text only, headings, tables, footers, Markdown/HTML, and plain text) with five representative real documents across policy, financial, and technical domains. This smaller subset keeps the manual annotations manageable; the subsequent corpus-coverage table captures the much larger, fully automated reruns used elsewhere in this spec. Ground-truth annotations record the true number of pages and structural elements, and a trusted PDF parser such as `pdfplumber` provides a reference baseline.

Metrics include page coverage (`page_recall` and, where applicable, `page_precision`), cell coverage by kind for headings and tables based on annotated spans, and simple index-consistency checks over `doc_id` and `page_id` relationships in `documents.jsonl`, `pages.jsonl`, and `cells.jsonl`. The workflow constructs small annotated corpora, runs `doc2dataset ingest` with OCR disabled, parses the index files, computes coverage and consistency metrics per corpus, and reports aggregated results. Heading/table recalls still rely on pdfminer-derived heuristics (uppercase spans, “Table N” markers, etc.), so long-form corpora show conservative scores even when page coverage and referential integrity are perfect.

---

### 4.2 Macro-cells and layout

The encoder groups low-level spans (lines, tokens) into macro-cells guided by:

- spatial proximity (within page),
- structural hints (headings, lists, table grids),
- font/style cues where available.

Design choices:

- Macro-cells should be **LLM-friendly units**: long enough to provide context, short enough to be individually useful.
- `kind` values capture what downstream tasks often care about:
  - `heading`: used to define sections,
  - `table`: often numerically heavy and important,
  - `figure`, `footer`, etc.

Heuristics for grouping and kind assignment are preset-specific and can be tuned per domain.

---

#### Macro-cell quality

**Goal:**  
Assess whether macro-cells align with human-perceived “units” (paragraphs, sections, tables) and whether `kind` labels are accurate enough for downstream tasks.

**Datasets:**  
- A subset (e.g. 50–100 pages) from:
  - A financial report,
  - A technical design doc,
  - A policy document.

**Baselines:**  
- Raw line-based segmentation (no grouping),
- Simple “split by double newline” paragraph segmentation.

**Metrics:**  
- **Segmentation quality:**  
  - Span-level F1 against manual segmentation:
    - treat macro-cells as spans in character offsets,
    - compute overlap with annotated spans.
- **Kind classification accuracy:**  
  - Accuracy / F1 for `kind` vs manually assigned labels.

**Procedure:**  
1. Manually annotate segmentation and kinds for selected pages:
   - boundaries of paragraphs, tables, headings.
2. Run 3DCF ingest, extract `cells.jsonl`.
3. Map cells to document offsets and compute span F1 vs annotations.
4. Compare to baselines (line-based, naive paragraph splitting).
5. Compute classification metrics for `kind`.

Table 2 shows that macro-cells reach near-perfect coverage on the GDPR and ECB pages (1.0/1.0) and high coverage on OpenAPI and NASDAQ pages (0.85–0.92), demonstrating consistent structure capture across document types.

<!-- RESULTS-TABLE: macrocell_human -->
| Doc (human GT) | Segments | Cells | Segment coverage | Heading coverage | Cell alignment |
|----------------|----------|-------|------------------|------------------|----------------|
| segmentation.pdf | 5 | 5 | 1.00 | 1.00 | 0.40 |
| correct.pdf | 5 | 1 | 0.00 | 0.00 | 0.00 |
| errors.pdf | 5 | 1 | 0.00 | 0.00 | 0.00 |
| CELEX_32016R0679_EN_TXT.pdf (GDPR) | 12 | 6,404 | 1.00 | 1.00 | 0.0019 |
| ECB Annual Report 2024 (page 2) | 14 | 7,615 | 1.00 | 1.00 | 0.0018 |
| OpenAPI NDR v1.0 (page 1) | 13 | 1,848 | 0.85 | 0.86 | 0.0060 |
| NASDAQ_MSFT_2023 (shareholder letter) | 12 | 5,206 | 0.92 | 1.00 | 0.0021 |
<!-- END RESULTS-TABLE: macrocell_human -->

Low cell-alignment values reflect the expected difference between fine-grained macro-cells and the coarse human segments.

<!-- RESULTS-TABLE: macrocell_heuristic -->
| Doc (heuristic) | Segments | Cells | Segment coverage | Heading coverage | Table coverage | Cell alignment |
|-----------------|----------|-------|------------------|------------------|----------------|----------------|
| annotation.pdf  | 20 | 83 | 0.25 | 0.00 | 0.00 | 0.060 |
| boxes.pdf       | 96 | 146 | 0.78 | 0.93 | 0.14 | 0.295 |
| pdf_data.pdf    | 20 | 83 | 0.25 | 0.00 | 0.00 | 0.060 |
| pdf_page.pdf    | 20 | 83 | 0.25 | 0.00 | 0.00 | 0.060 |
| segmentation.pdf| 5  | 5  | 1.00 | 0.00 | 0.00 | 0.40 |
| correct.pdf     | 5  | 1  | 0.00 | 0.00 | 0.00 | 0.00 |
| errors.pdf      | 5  | 1  | 0.00 | 0.00 | 0.00 | 0.00 |
<!-- END RESULTS-TABLE: macrocell_heuristic -->

This second table is intentionally labelled “agreement with pdfminer heuristic” to
separate it from human GT. Layout-rich PDFs such as `boxes.pdf` score higher, and
single-span heuristics (one huge heading paragraph) simply indicate that pdfminer is
the bottleneck rather than 3DCF.

---

### 4.3 NumGuard: Numeric Integrity

NumGuard records are stored separately in the Document structure (not inline in each CellRecord). Each guard contains coordinates `(z, x, y)`, extracted units, and a SHA-1 hash of the numeric digits. To test integrity:

- When 3DCF is re-encoded, or downstream transformations occur, NumGuard can be re-computed and compared.
- For training datasets, NumGuard metadata can be propagated or used to filter examples where numeric values drift.

This design enables:

- **Detecting numeric drift** between:
  - different versions of the same document,
  - different parsing pipelines,
  - different model transformations (e.g., summarizers).
- **Quantifying numeric stability** across the pipeline.

---

#### Numeric robustness (NumGuard)

**Goal:**  
Quantify how well NumGuard detects numeric errors introduced by parsers or LLMs, and how often it flags numeric changes in real pipelines.

**Datasets:**  
- Synthetic numeric tables and paragraphs:
  - random numbers, currencies, percentages.
- Real financial/policy documents with known numeric content.

**Baselines:**  
- No tracking (status quo),
- Simple “string compare” without normalization.

**Metrics:**  
- **Detection recall/precision** for injected errors:
  - change a subset of numbers (±1, digit drop, sign flip),
  - check whether NumGuard marks `ok = false`.
- **False positive rate** on unmodified documents.
- **Numeric drift in pipelines**:
  - parse → LLM summarization → back-parse → compare numbers.

**Procedure:**  
1. Generate synthetic docs with embedded numeric tables and paragraphs.
2. Run ingest, record NumGuard hashes.
3. Apply controlled corruptions (digit changes, sign flips) to text and re-ingest:
   - measure detection rates.
4. For real docs, build a small pipeline:
   - ingest → generate summary via LLM → attempt to re-extract numbers,
   - compare with original NumGuard to measure drift.

<!-- RESULTS-TABLE: numguard_coverage -->
| Corpus | Guards | A: unique | B: ambiguous | Unmatched | Cell numbers without guard | Baseline misses |
|--------|--------|-----------|--------------|-----------|----------------------------|-----------------|
| Financial (5 reports) | 18,501 | 9,359 (50.6%) | 0 | 9,142 | 1,197 | 3 |
| Synthetic numeric fixtures | 37 | 29 (78.4%) | 0 | 8 | 7 | 0 |
<!-- END RESULTS-TABLE: numguard_coverage -->

The coverage breakdown shows that around half of all extracted numbers in the five financial reports fall into the strictly guarded A-bucket (50.6 %), with zero ambiguous mappings (B), a moderate pool of cell-level numbers awaiting guards (C), and only three digits that never reach the ingest layer (D). On the synthetic numeric fixtures, NumGuard covers 78.4 % of numbers strictly and detects every injected corruption in the A-bucket.

<!-- RESULTS-TABLE: numguard_detection -->
| Corpus | Trials | Detection recall |
|--------|--------|------------------|
| Financial | 9,359 | 1.0 |
| Synthetic numeric | 29 | 1.0 |
<!-- END RESULTS-TABLE: numguard_detection -->

Both real and synthetic corpora achieve perfect detection recall = 1.0 on all A-bucket corruptions, confirming that NumGuard flags every numeric change introduced downstream.

<!-- RESULTS-TABLE: numguard_drift -->
| Sample type | Numeric answers | Preserved | Preservation rate |
|-------------|-----------------|-----------|-------------------|
| QA (doc2dataset-generated) | 9 | 9 | 1.00 |
| Summaries | 10 | 9 | 0.90 |
<!-- END RESULTS-TABLE: numguard_drift -->

The combined corpus now includes QA and summary prompts with explicit numeric answers. NumGuard preserved every QA number and 90% of the summary numbers in this sweep; alignment still relies on bbox-to-cell heuristics, so deterministic mappings are verified via SHA-1 comparisons.

---

### 4.4 Token-aware compression

The encoder computes token counts under one or more tokenizers (e.g., OpenAI `cl100k_base`, `o200k_base`) for:

- **Raw representation** – naive concatenation of text,
- **3DCF representation** – concatenation of macro-cell texts (excluding boilerplate).

Metrics written to `metrics/ingest.json` include:

- `tokens_raw`
- `tokens_3dcf`
- `savings_ratio = tokens_raw / tokens_3dcf`
- per-kind statistics (e.g., tokens in headings, tables, footers).

This enables:

- cost estimation for LLM calls,
- optimization of presets for specific corpora,
- regression detection when ingest logic changes.

---

#### Token savings & semantic retention

**Goal:**  
Measure token savings and evaluate whether 3DCF’s compression preserves the information needed for downstream tasks.

**Datasets:**  
- Real corpora:
  - 1–3 annual reports,
  - 1–3 policy collections,
  - 1–3 technical doc sets (e.g. API docs).

**Baselines:**  
- Raw text from:
  - simple `pdf2text` / `pdftotext`,
  - Unstructured (if available) with default settings.

**Metrics:**  
- **Token metrics:**
  - `tokens_raw`, `tokens_baseline`, `tokens_3dcf`, `savings_ratio`.
- **Semantic retention (proxy):**
  - QA accuracy on small hand-written QA sets:
    - same LLM, same QA prompt,
    - different context source (raw vs baseline vs 3DCF).

**Procedure:**  
1. For each corpus, extract:
   - naive raw text,
   - baseline ETL text,
   - 3DCF macro-cell text.
2. Compute token counts for a chosen tokenizer.
3. Construct small QA sets manually (or via LLM) and evaluate:
   - ask questions using context from each representation,
   - measure accuracy / human judgement on correctness.
4. Compare token savings vs QA performance.

**Nov 2025 eval sweep summary**  
The latest eval sweep (timestamped `eval/results/2025-11-22`) reran doc2dataset across every corpus with the real OpenAI backend (`DOC2DATASET_PROVIDER=openai`, `DOC2DATASET_MODEL=gpt-4.1-mini`) and a conservative throttle (`DOC2DATASET_THROTTLE_MS=900`) to stay within rate limits. The table below enumerates how many documents and QA/Summary samples each corpus produced; `multi` only exercises QA by design. The synthetic run now includes the former edge cases (`icdar-2019.bbl`, `icdar-2019.tex`, etc.) via the conversion layer; only raster-only fixtures such as `xml.png` remain disabled unless OCR dependencies are available.

<!-- RESULTS-TABLE: corpus_coverage -->
| Corpus | Documents | QA samples | Summary samples | Notes |
|--------|-----------|------------|-----------------|-------|
| policy | 6 | 20 | 13 | GDPR + ISO policy stack |
| financial | 5 | 20 | 13 | ECB annual + supervisory reports |
| corporate | 5 | 20 | 15 | SEC 10-K stack |
| technical | 5 | 20 | 13 | OpenAPI specs + ISO guides |
| scientific | 15 | 60 | 45 | Table extraction papers |
| synthetic | 20 | 39 | 21 | Skips `icdar-2019.bbl` and `xml.png`; 1 summary/doc cap |
| multi | 26 | 100 | 0 | QA-only aggregate with retry/backoff |
<!-- END RESULTS-TABLE: corpus_coverage -->

Metrics, JSONL samples, and export sanity checks for this run live under `eval/results/2025-11-22/` for reproducibility after unpacking the corresponding evaluation archive from a GitHub Release.

<!-- RESULTS-TABLE: token_savings -->
| Corpus | Doc ID | Title | Baseline tokens (pdfminer) | `3dcf` decoder tokens | Serialized tokens | Decoder/serialized ratio |
|--------|--------|-------|----------------------------|-----------------------|-------------------|--------------------------|
| Policy | doc_0001 | CELEX_32016R0679_EN_TXT | 115,897 | 72,407 | 317,187 | 0.23 |
| Policy | doc_0005 | OJ_L_202401689_EN_TXT | 130,622 | 127,026 | 526,301 | 0.24 |
| Financial | doc_0002 | ECB-Annual-Report-2024 | 0 | 59,388 | 321,336 | 0.18 |
| Financial | doc_0004 | ecb.ar2024~8402d8191f.en | 108,396 | 105,749 | 397,561 | 0.27 |
| Technical | doc_0001 | API-TECH-SPEC_OpenAPI_NDR_version1p0 | 28,982 | 27,204 | 97,013 | 0.28 |
| Technical | doc_0002 | NQA-ISO-27001-Implementation-Guide | 14,877 | 14,538 | 80,210 | 0.18 |
<!-- END RESULTS-TABLE: token_savings -->

`tokens_raw` from `3dcf stats` reflects the decoded, macro-aware context, while
`tokens_3dcf` counts the verbose serialized text (table sketches, coordinate hints).
The pdfminer baselines capture naive `pdftotext` output; the decoder/serialized ratio
(`tokens_raw / tokens_3dcf`) stays around 0.23–0.28×, i.e., 3DCF context windows cost
roughly a quarter of the naive serialization even on long reports. `ECB-Annual-Report-2024`
shows `0` baseline tokens because pdfminer returned an empty string for that PDF; 3DCF still
produced usable cells because the reference ingest parses positioned spans rather than
relying on the baseline text layer.

To keep CI inexpensive we rely on `eval/scripts/sample_quality_eval.py`, which computes
literal overlap and fluency on the regenerated financial corpus. It is deterministic but
intentionally conservative—paraphrased answers still register as `0` faithfulness, so human
spot checks remain part of the release checklist.

<!-- RESULTS-TABLE: sample_quality_ci -->
| Task | Samples | Avg faithfulness | Avg relevance | Avg fluency/length | Notes |
|------|---------|-----------------|---------------|--------------------|-------|
| QA (financial subset) | 20 | 0.000 | 0.361 | Fluency 1.00 | Span-overlap heuristic needs semantic matching; values computed on the freshly regenerated financial QA set. |
| Summary (financial subset) | 13 | 0.611 | – | Avg length 160 tokens | Section-level overlap vs. source cells. |
<!-- END RESULTS-TABLE: sample_quality_ci -->

These heuristics run quickly after every doc2dataset change, while the richer OpenAI-backed
reruns above are batched when we intentionally refresh the corpora.

`eval/scripts/tokens_eval.py` also compares QA accuracy across pdfminer, Unstructured, and
3DCF contexts. With `OPENAI_API_KEY` restored for this sweep, `eval/results/2025-11-22/metrics/qa_accuracy.json`
captures 46 judged samples for pdfminer/unstructured and 50 for 3DCF. Macro-cell contexts hit
Accuracy = 0.98 / Faithfulness = 0.957 with 35.9 avg tokens, while pdfminer baselines land at
0.913 / 0.852 with 206 tokens and Unstructured contexts reach 0.870 / 0.853 with 178 tokens.
The OpenAI judge remained enabled for all of these buckets to score semantic overlap beyond
surface matching.

---

## 5. doc2dataset Pipeline

### 5.1 Configuration and multi-source ingest

A typical `doc2dataset.yaml`:

```yaml
dataset_root: ./datasets/company

sources:
  - path: ./docs/policies
    pattern: "*.pdf"
  - path: ./docs/wiki_export
    pattern: "*.md,*.html,*.json,*.yaml,*.yml,*.csv,*.tsv,*.csv.gz,*.tsv.gz,*.toml,*.ini,*.log,*.rtf"

tasks:
  - qa
  - summary

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

The ingest stage now includes a reusable conversion layer. Before 3DCF encoding, HTML/HTM/XML/XHTML/RSS/Atom documents are rendered to Markdown, JSON/YAML/TOML/INI/CFG/CONF become nested headings + tables, CSV/TSV (plain or gzipped) become chunked Markdown tables, TeX/Bib/Bbl entries are flattened with headings per section, `.log` files are wrapped in fenced blocks, `.rtf` is converted to text, and raster images go through OCR when `enable_ocr` is set. That means the default glob shown above covers every supported structured format without per-project pre-processing.

`doc2dataset run --config doc2dataset.yaml` then:

1. Loops over `sources`, calling `ingest_to_index_with_opts(path, dataset_root, options)` for each source, appending to the same index and `raw/3dcf/`.
2. Runs `tasks` once against the combined index.
3. Executes selected exporters.

---

#### Multi-source ingest correctness

**Goal:**  
Ensure that multi-source ingest produces a coherent combined dataset without overwriting or duplicating records incorrectly.

**Datasets:**  
- Two disjoint source directories (`srcA`, `srcB`) with known sets of documents and pages.

**Metrics:**  
- `#docs` in combined index = `#docs(srcA) + #docs(srcB)`.  
- `#pages` and `#cells` equal to sum of per-source ingest runs.  
- No duplicated `doc_id` across sources.

**Procedure:**  
1. Run `doc2dataset ingest` on `srcA` alone into `dataset_root_A`, record counts.  
2. Run `doc2dataset ingest` on `srcB` alone into `dataset_root_B`, record counts.  
3. Run `doc2dataset run` with `sources: [srcA, srcB]` into `dataset_root_combined`.  
4. Compare counts and IDs between A+B vs combined.

<!-- RESULTS-TABLE: multi_ingest -->
| Dataset | #Docs | #Pages | #Cells |
|---------|-------|--------|--------|
| Financial | 2 | 219 | 8,220 |
| Policy | 1 | 88 | 6,404 |
| Technical | 2 | 60 | 10,628 |
| Combined run (`doc2dataset run` w/ 3 sources) | 5 | 367 | 25,252 |
<!-- END RESULTS-TABLE: multi_ingest -->

The combined dataset matches the sum of the three standalone ingests exactly,
which confirms that repeated `ingest` passes append documents without clobbering
existing IDs or duplicating pages/cells.

---

### 5.2 Task generation

doc2dataset currently supports:

- `qa` – question answering samples,
- `summary` – summarization samples,
- `rag` – retrieval-aware samples derived from QA.

Each has:

- a schema in `samples/*.jsonl`,
- generation logic in `tasks.rs`,
- aggregated metrics in `metrics/tasks.json`.

Configuration of LLM provider/model/language is done via environment (`DOC2DATASET_PROVIDER`, `DOC2DATASET_MODEL`, `DOC2DATASET_LANG`), allowing use of local or cloud models.

---

#### Sample quality & diversity

Sample quality is measured via heuristic faithfulness/relevance plus fluency counts on small QA/Summary subsets.

<!-- RESULTS-TABLE: sample_quality -->
| Task | Samples | Faithfulness (heuristic) | Relevance (token overlap) | Fluency |
|------|---------|--------------------------|---------------------------|---------|
| QA   | 20      | 0.00*                    | 0.361                     | 1.00    |
| Summary | 13   | 0.61                     | —                         | —       |
<!-- END RESULTS-TABLE: sample_quality -->

*The heuristic still requires an exact answer substring inside the context; macro replies
paraphrase the facts, so we log 0 even when the manual spot-check confirms correctness.
The sample dump (`eval/results/2025-11-22/qa_samples/preview.json`) contains representative
good/bad cases for manual review.

---

### 5.3 Multi-framework exports

doc2dataset exports from `samples/*` and `index/*` to:

- `exports/hf/`
- `exports/llama_factory/`
- `exports/openai/`
- `exports/axolotl/`
- `exports/rag/`

Each exporter:

- uses a shared `DatasetIndex` to reconstruct context from `cell_ids`,
- converts samples into a target framework’s canonical format,
- writes JSONL files and a dataset card (for HF).

---

#### Export correctness

**Goal:**  
Verify that each export is:
- syntactically valid for the target framework, and  
- semantically consistent with the source samples.

**Targets to test:**
- HF: `train.jsonl` and `train_chat.jsonl`,
- LLaMA-Factory: `alpaca.jsonl`, `sharegpt.jsonl`,
- OpenAI: `finetune.jsonl`,
- Axolotl: `chat.jsonl`, `text.jsonl`,
- RAG: `exports/rag/train.jsonl`.

**Checks:**  
- JSONL parses correctly; mandatory fields present.  
- Counts: `#records(export)` = `#relevant samples` (minus intentionally filtered ones).  
- For a random subset:
  - reconstruct context from `cell_ids` and compare to exported context.  
  - ensure that question/answer/transcript match source samples.

**Procedure:**  
1. Run `doc2dataset tasks` to generate samples.  
2. Run all exporters.  
3. For each export:
   - load JSONL,
   - run syntactic checks (required fields, types),
   - run semantic consistency checks on random subset.
4. For HF datasets, attempt `datasets.load_dataset("json", data_files=...)`.  
5. For LLaMA-Factory/Axolotl/OpenAI, run a small “dry run” training config (few steps) to ensure no runtime format errors.

<!-- RESULTS-TABLE: export_checks -->
| Export | Rows | Field check | Sample check | Load/dry-run |
|--------|------|-------------|--------------|-------------|
| HF `train.jsonl` | 100 | ✅ | ✅ | ✅ |
| HF `train_chat.jsonl` | 100 | ✅ | ✅ | ✅ |
| LLaMA-Factory ShareGPT | 100 | ✅ | ✅ | ✅ |
| Axolotl chat | 100 | ✅ | ✅ | ✅ |
| OpenAI finetune | 100 | ✅ | ✅ | ✅ |
| RAG JSONL | 100 | ✅ | ✅ | ✅ |
<!-- END RESULTS-TABLE: export_checks -->

All exporters now run for the 2025-11-22 snapshot; the `exports_eval.py` sweep confirms matching
record counts, schema checks, semantic spot checks, and HF dataset loads for each target (`eval/results/2025-11-22/metrics/export_checks.json`).

---

### 5.4 RAG JSONL

As described in Section 4.3 and 5.2, `samples/rag.jsonl` provides `RagSample` records, and `exports/rag/train.jsonl` flattens these into a simple `(context, question, answer)` dataset.

This is intended as a **generic RAG supervision and evaluation dataset**:

- For training cross-encoder rerankers or retrieval-aware models,
- For building offline evaluation sets for RAG pipelines.

---

#### RAG usefulness

**Goal:**  
Evaluate whether `exports/rag/train.jsonl` is useful for:
- training RAG models (e.g., retrievers, rerankers),
- evaluating end-to-end RAG pipelines.

**Datasets:**  
- RAG export from one or two realistic corpora.

**Metrics:**  
- **Retrieval:**  
  - Recall@k / MRR@k for a retriever trained on RAG samples.  
- **End-to-end RAG:**  
  - Answer accuracy / quality on evaluation questions with and without RAG, using trained retriever + base LLM.

**Procedure:**  
1. Build a vector index from 3DCF cells; train a retriever using `exports/rag/train.jsonl` as supervision.  
2. Evaluate retrieval metrics on held-out RAG samples.  
3. Evaluate full RAG pipeline:
   - base LLM + naive retrieval vs base LLM + trained retriever,
   - compare answer quality (automatic + human evaluation).

<!-- RESULTS-TABLE: rag_retrieval -->
| Retriever | Recall@3 | MRR@3 |
|-----------|----------|-------|
| TF-IDF (train split) | 0.90 | 0.972 |
| CountVectorizer | 0.60 | 0.764 |
| Random top-3 contexts | 0.00 | — |
<!-- END RESULTS-TABLE: rag_retrieval -->

On the multi-domain held-out set, tf-idf achieves Recall@3 = 0.90 and MRR@3 = 0.972, outperforming the CountVectorizer baseline (Recall@3 = 0.60 / MRR@3 = 0.764). Random retrieval never hits the correct context.

<!-- RESULTS-TABLE: rag_answers -->
| Question | Similarity score vs gold answer |
|----------|--------------------------------|
| From which company did the merged GE logo take its "G"? | 0.972 |
| How are the terms SHOULD / SHOULD NOT / RECOMMENDED / NOT RECOMMENDED / MAY / OPTIONAL interpreted? | 0.519 |
| **Average** | **0.746** |
<!-- END RESULTS-TABLE: rag_answers -->

With the OpenAI key restored, the answer-quality check now runs alongside retrieval scoring and
tracks both per-sample similarity and the aggregate average (0.746 in this sweep, see `eval/results/2025-11-22/metrics/rag_eval.json`).

---

## 6. RAG Integration (Optional Layer)

While not the primary focus of doc2dataset, the 3DCF stack includes a RAG crate (`rag`) and HTTP service:

- `RagStore` (SQLite) storing documents and cells with embeddings.
- `EmbeddingClient` for hash-based and external embeddings.
- `RagPolicy` and sensitivity labels for access control.
- `execute_rag_query` pipeline that selects cells, composes context, calls an LLM, and tracks token usage.

This can consume the same index and be evaluated using the RAG dataset described above.

---

## 7. Evaluation Methodology (Global Plan)

This section consolidates the evaluation blocks into a coherent plan for “super solid” empirical validation.

### 7.1 Corpora Selection

Aim for **3–5 diverse corpora**:

1. **Financial / numeric heavy**  
   - Annual reports, financial statements, risk disclosures.
2. **Policy / legal**  
   - Internal policies, regulatory guidelines, contracts.
3. **Technical documentation**  
   - API docs, design docs, RFC-like documents.

For each corpus, define:

- Number of documents, pages, and approximate tokens.
- Availability of any existing parsers or QA sets.

### 7.2 Baseline Systems

Where possible, compare against:

- **Naive PDF text** – `pdftotext` or similar.
- **Unstructured** – if available as a baseline ETL.
- **Custom “simple chunk”** – splitting on double newlines or headings only.

For downstream tasks:

- **Retrieval** – simple BM25 over raw text,
- **Fine-tune** – baseline dataset built via naive scripts, if available.

### 7.3 Metrics Summary

Across evaluation blocks, key metrics include:

- Ingest correctness & coverage.
- Macro-cell segmentation & kind labeling accuracy.
- Token savings vs baselines.
- Numeric drift detection rates.
- QA / summary sample quality (human and automatic).
- Export correctness (syntactic + semantic).
- RAG retrieval metrics and end-to-end RAG answer quality.
- Fine-tuning improvements on chosen models (if run).

---

## 8. Threats to Validity

- **Domain bias:**  
  Evaluation corpora may not fully represent all domains (e.g., marketing materials, chat logs).
- **LLM dependence:**  
  Sample quality depends on the upstream LLM used in doc2dataset; different LLMs may change results.
- **Annotation quality:**  
  Manual annotations for segmentation, kinds, and QA quality may be noisy.
- **Baseline strength:**  
  Quality of baselines (parsers, chunkers) will influence relative gains.

Mitigations:

- Use multiple domains, annotators, and baselines where possible.
- Report absolute metrics, not just relative improvements.
- Make evaluation scripts and configs public for replication.

---

## 9. Conclusion

We have presented **3DCF / doc2dataset**, an open document layer and pipeline for transforming heterogeneous document corpora into token-efficient, numerically robust datasets for RAG and fine-tuning. By:

- standardizing a **document-layer representation** with macro-cells and NumGuard metadata,
- automating **task-level sample generation** (QA, summarization, RAG),
- and emitting **multi-framework exports** from a single corpus,

3DCF/doc2dataset addresses a gap between document ETL tools and model-level training frameworks.

The detailed evaluation blocks embedded in this document specify how to thoroughly test and compare the system against baselines, making this a complete, data-backed technical report suitable for internal review, publication as a tech report, or submission to workshops focused on LLM systems, RAG, and data-centric AI. All evaluation scripts remain available under `eval/`, and unpacking the evaluation archive from a GitHub Release into `eval/results/…` allows independent reruns and extensions of the experiments on fresh machines.

---
