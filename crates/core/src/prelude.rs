//! Prelude module for convenient imports.
//!
//! ```rust
//! use three_dcf_core::prelude::*;
//! ```
//!
//! This brings the most commonly used types into scope for quick prototypes.

// Core encoder types
pub use crate::encoder::{EncodeInput, Encoder, EncoderBuilder, EncoderPreset};

// Document types
pub use crate::document::{CellRecord, CellType, CodeHash, Document, Header, PageInfo};

// Serialization
pub use crate::serializer::{TableMode, TextSerializer, TextSerializerConfig};

// Chunking for RAG
pub use crate::chunk::{ChunkConfig, ChunkMode, ChunkRecord, Chunker};

// Configuration types
pub use crate::normalization::{HyphenationMode, ImportanceTuning};

// Error handling
pub use crate::error::{DcfError, Result};

// Metrics
pub use crate::metrics::Metrics;

// Index types for JSONL export
pub use crate::index::{DocumentRecord, JsonlWriter, PageRecord};

// Stats
pub use crate::stats::{estimate_tokens, TokenizerKind};
