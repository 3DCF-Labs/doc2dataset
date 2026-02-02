//! # three-dcf-core
//!
//! A high-performance library for encoding documents into structured datasets
//! optimized for LLM training and retrieval-augmented generation (RAG).
//!
//! ## Overview
//!
//! `three-dcf-core` converts various document formats (PDF, Markdown, HTML, images)
//! into a normalized, cell-based representation that preserves document structure
//! while being optimized for machine learning workloads.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use three_dcf_core::prelude::*;
//!
//! fn main() -> Result<()> {
//!     // Encode a PDF document
//!     let encoder = Encoder::from_preset("reports")?;
//!     let (document, metrics) = encoder.encode_path("report.pdf")?;
//!
//!     println!("Processed {} pages, {} cells", metrics.pages, metrics.cells_kept);
//!
//!     // Serialize to text format for LLM context
//!     let serializer = TextSerializer::new();
//!     let output = serializer.to_string(&document)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Encoder Presets
//!
//! | Preset | Use Case | Page Size |
//! |--------|----------|-----------|
//! | `reports` | Business documents, papers | 1024×1400 |
//! | `slides` | Presentations | 1920×1080 |
//! | `news` | Articles, blogs | 1100×1600 |
//! | `scans` | Scanned documents | 1400×2000 |
//!
//! ## Features
//!
//! - **`text`** (default): Basic text/markdown/HTML processing
//! - **`pdfium`**: Native PDF rendering via pdfium for better extraction
//! - **`ocr`**: Optical character recognition via Tesseract
//! - **`full`**: All features enabled
//!
//! ## Architecture
//!
//! The encoding pipeline:
//!
//! 1. **Input** → Document loaded from file (PDF/MD/HTML/image)
//! 2. **Parse** → Extract pages and text content
//! 3. **Normalize** → Apply hyphenation rules, detect structure
//! 4. **Classify** → Identify cell types (text, table, code, header)
//! 5. **Score** → Calculate importance scores for ranking
//! 6. **Deduplicate** → Hash-based deduplication across pages
//! 7. **Output** → `Document` with cells, dictionary, and metadata
//!
//! ## Output Formats
//!
//! - **TextSerializer**: Human-readable format for LLM context windows
//! - **JsonlWriter**: JSONL output for dataset pipelines
//! - **Protobuf**: Binary format via `proto` module
//!
//! ## Example: Custom Configuration
//!
//! ```rust,no_run
//! use three_dcf_core::{EncoderBuilder, HyphenationMode, ImportanceTuning};
//!
//! let encoder = EncoderBuilder::new("reports")?
//!     .budget(Some(4096))           // Token budget
//!     .drop_footers(true)           // Remove page footers
//!     .dedup_window(5)              // Dedup across 5 pages
//!     .hyphenation(HyphenationMode::Preserve)
//!     .importance_tuning(ImportanceTuning {
//!         header_boost: 1.5,
//!         table_boost: 1.2,
//!         ..Default::default()
//!     })
//!     .build();
//! # Ok::<(), three_dcf_core::DcfError>(())
//! ```
//!
//! ## Chunking for RAG
//!
//! ```rust,no_run
//! use three_dcf_core::{Chunker, ChunkConfig, ChunkMode};
//!
//! let chunker = Chunker::new(ChunkConfig {
//!     mode: ChunkMode::Semantic,
//!     target_tokens: 512,
//!     overlap_tokens: 64,
//!     ..Default::default()
//! });
//!
//! let chunks = chunker.chunk(&document);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

/// Protobuf-generated types for binary serialization
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/dcf.v1.rs"));
}

/// Index types for JSONL output (merged from three_dcf_index)
pub mod index;

/// Prelude for convenient imports
pub mod prelude;

mod bench;
mod chunk;
mod decoder;
mod document;
mod embedding;
mod encoder;
mod error;
mod ingest;
mod metrics;
mod normalization;
mod numguard;
mod ocr;
mod serializer;
mod stats;

// Re-exports for public API
pub use bench::{BenchConfig, BenchMode, BenchResult, BenchRunner, CorpusMetrics};
pub use chunk::{ChunkConfig, ChunkMode, ChunkRecord, Chunker};
pub use decoder::Decoder;
pub use document::{
    hash_payload, CellRecord, CellType, CodeHash, Document, Header, NumGuard, NumGuardAlert,
    NumGuardIssue, PageInfo,
};
pub use embedding::{EmbeddingRecord, HashEmbedder, HashEmbedderConfig};
pub use encoder::{EncodeInput, Encoder, EncoderBuilder, EncoderPreset};
pub use error::{DcfError, Result};
pub use ingest::{ingest_to_index, ingest_to_index_with_opts, IngestOptions};
pub use metrics::{cer, numeric_stats, wer, Metrics, NumStats, TokenMetrics};
pub use normalization::{HyphenationMode, ImportanceTuning};
pub use serializer::{TableMode, TextSerializer, TextSerializerConfig};
pub use stats::{estimate_tokens, Stats, TokenizerKind};

// Re-export index types at crate root for convenience
pub use index::{DocumentRecord, JsonlWriter, PageRecord};
// Note: index::CellRecord conflicts with document::CellRecord; refer to it via `index::CellRecord` explicitly.
