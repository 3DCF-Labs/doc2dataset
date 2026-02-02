# three-dcf-core

[![Crates.io](https://img.shields.io/crates/v/three-dcf-core.svg)](https://crates.io/crates/three-dcf-core)
[![Documentation](https://docs.rs/three-dcf-core/badge.svg)](https://docs.rs/three-dcf-core)
[![License](https://img.shields.io/crates/l/three-dcf-core.svg)](LICENSE)

A high-performance Rust library for encoding documents into structured datasets optimized for LLM training and retrieval-augmented generation (RAG).

## Features

- **Multi-format support**: PDF, Markdown, HTML, plain text, and images
- **Structure preservation**: Maintains document hierarchy (headers, tables, code blocks)
- **Token-aware**: Budget-based encoding with tiktoken-compatible token counting
- **Deduplication**: Hash-based content deduplication across pages
- **NumGuard**: Automatic numerical data validation
- **Parallel processing**: Rayon-powered multi-threaded encoding
- **Optional OCR**: Tesseract integration for scanned documents

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
three-dcf-core = "0.2"
```

For PDF rendering and OCR support:

```toml
[dependencies]
three-dcf-core = { version = "0.2", features = ["full"] }
```

## Quick Start

```rust
use three_dcf_core::prelude::*;

fn main() -> Result<()> {
    // Encode a PDF with the "reports" preset
    let encoder = Encoder::from_preset("reports")?;
    let (document, metrics) = encoder.encode_path("quarterly_report.pdf")?;

    println!("Pages: {}, Cells: {}", metrics.pages, metrics.cells_kept);

    // Serialize for LLM context
    let serializer = TextSerializer::new();
    let context = serializer.to_string(&document)?;

    // Or export to JSONL for ML pipelines
    let mut writer = JsonlWriter::new(std::fs::File::create("output.jsonl")?);
    writer.write_record(&DocumentRecord {
        doc_id: "doc_001".to_string(),
        title: Some("Q4 Report".to_string()),
        source_type: "files".to_string(),
        source_format: "pdf".to_string(),
        source_ref: "quarterly_report.pdf".to_string(),
        tags: vec!["finance".to_string()],
    })?;

    Ok(())
}
```

## Encoder Presets

| Preset | Use Case | Resolution |
|--------|----------|------------|
| `reports` | Business documents, research papers | 1024×1400 |
| `slides` | Presentations | 1920×1080 |
| `news` | Articles, blog posts | 1100×1600 |
| `scans` | Scanned documents | 1400×2000 |

## Advanced Configuration

```rust
use three_dcf_core::{EncoderBuilder, HyphenationMode, ImportanceTuning};

let encoder = EncoderBuilder::new("reports")?
    .budget(Some(4096))           // Limit to 4K tokens
    .drop_footers(true)           // Remove page footers
    .dedup_window(5)              // Dedup across 5 consecutive pages
    .hyphenation(HyphenationMode::Merge)
    .importance_tuning(ImportanceTuning {
        header_boost: 1.5,
        table_boost: 1.2,
        ..Default::default()
    })
    .build();
```

## Chunking for RAG

```rust
use three_dcf_core::{Chunker, ChunkConfig, ChunkMode};

let chunker = Chunker::new(ChunkConfig {
    mode: ChunkMode::Semantic,
    target_tokens: 512,
    overlap_tokens: 64,
    ..Default::default()
});

let chunks = chunker.chunk(&document);
for chunk in chunks {
    println!("Chunk: {} tokens", chunk.approx_tokens);
}
```

## Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `text` | Basic text processing (default) | - |
| `pdfium` | Native PDF rendering | `pdfium-render` |
| `ocr` | Tesseract OCR | `leptess` |
| `full` | All features | All above |

## Output Formats

### Text Serialization (for LLM context)

```
<ctx3d grid=coarse codeset=HASH256 preset=reports budget=auto>
(z=0,x=64,y=64,w=896,h=24,code=a1b2c3d4...,rle=0,imp=100,type=HEADER) "Quarterly Results"
(z=0,x=64,y=100,w=896,h=200,code=e5f6g7h8...,rle=0,imp=80,type=TABLE) "[table rows=5 cols=4]"
</ctx3d>
```

### JSONL (for ML pipelines)

```json
{"doc_id":"doc_001","title":"Q4 Report","source_type":"files",...}
{"page_id":"page_001","doc_id":"doc_001","page_number":0,...}
{"cell_id":"cell_001","doc_id":"doc_001","kind":"header","text":"...","importance":1.0,...}
```

## Performance

Benchmarks on M2 MacBook Pro (8-core):

| Document Type | Pages | Time | Throughput |
|--------------|-------|------|------------|
| PDF (text) | 100 | 1.2s | 83 pages/s |
| PDF (scanned) | 100 | 45s | 2.2 pages/s |
| Markdown | 1000 | 0.8s | 1250 files/s |

## License

Apache-2.0

## Contributing

See [CONTRIBUTING.md](https://github.com/3DCF-Labs/doc2dataset/blob/main/CONTRIBUTING.md) for guidelines.
