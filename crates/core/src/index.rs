//! Index types for JSONL output and dataset pipelines.
//!
//! This module provides data structures for exporting documents to JSONL format,
//! suitable for downstream ML pipelines and vector databases.
//!
//! # Example
//!
//! ```rust,no_run
//! use three_dcf_core::index::{DocumentRecord, JsonlWriter};
//! use std::fs::File;
//!
//! let file = File::create("output.jsonl")?;
//! let mut writer = JsonlWriter::new(file);
//!
//! writer.write_record(&DocumentRecord {
//!     doc_id: "doc_001".to_string(),
//!     title: Some("Annual Report 2024".to_string()),
//!     source_type: "files".to_string(),
//!     source_format: "pdf".to_string(),
//!     source_ref: "/data/reports/annual_2024.pdf".to_string(),
//!     tags: vec!["finance".to_string(), "annual".to_string()],
//! })?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Metadata record for a processed document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentRecord {
    pub doc_id: String,
    pub title: Option<String>,
    pub source_type: String,
    pub source_format: String,
    pub source_ref: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Metadata record for a single page within a document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageRecord {
    pub page_id: String,
    pub doc_id: String,
    pub page_number: u32,
    pub approx_tokens: Option<u32>,
    #[serde(default)]
    pub meta: Value,
}

/// Record for a single cell (text block) within a page.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CellRecord {
    pub cell_id: String,
    pub doc_id: String,
    pub page_id: String,
    pub kind: String,
    pub text: String,
    pub importance: f32,
    pub bbox: Option<[f32; 4]>,
    pub numguard: Option<Value>,
    #[serde(default)]
    pub meta: Value,
}

/// A streaming JSONL writer for efficient dataset export.
pub struct JsonlWriter<W> {
    writer: W,
}

impl<W: Write> JsonlWriter<W> {
    /// Create a new JSONL writer wrapping the given writer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write a single record as a JSON line.
    pub fn write_record<T: Serialize>(&mut self, record: &T) -> Result<()> {
        let mut buf = serde_json::to_vec(record)?;
        buf.push(b'\n');
        self.writer.write_all(&buf)?;
        Ok(())
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Consume the writer and return the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_writer_roundtrips_records() {
        let record = DocumentRecord {
            doc_id: "doc_1".to_string(),
            title: Some("Test".to_string()),
            source_type: "files".to_string(),
            source_format: "pdf".to_string(),
            source_ref: "/tmp/input.pdf".to_string(),
            tags: vec!["tag1".to_string()],
        };
        let writer = Vec::new();
        let mut writer = JsonlWriter::new(writer);
        writer.write_record(&record).unwrap();
        let buf = writer.into_inner();
        assert!(buf.ends_with(b"\n"));
        let parsed: DocumentRecord = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed.doc_id, "doc_1");
        assert_eq!(parsed.title.unwrap(), "Test");
    }

    #[test]
    fn jsonl_writer_multiple_records() {
        let mut writer = JsonlWriter::new(Vec::new());

        writer
            .write_record(&DocumentRecord {
                doc_id: "doc_1".to_string(),
                ..Default::default()
            })
            .unwrap();
        writer
            .write_record(&DocumentRecord {
                doc_id: "doc_2".to_string(),
                ..Default::default()
            })
            .unwrap();

        let buf = writer.into_inner();
        let lines: Vec<&str> = std::str::from_utf8(&buf).unwrap().lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("doc_1"));
        assert!(lines[1].contains("doc_2"));
    }
}
