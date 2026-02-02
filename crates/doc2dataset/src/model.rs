use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use three_dcf_core::index::{CellRecord as IndexCellRecord, DocumentRecord};

#[derive(Debug, Clone, Copy)]
pub enum SourceType {
    Files,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceType::Files => "files",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileFormat {
    Pdf,
    Markdown,
    Txt,
    Html,
    Xml,
    Json,
    Yaml,
    Csv,
    Tsv,
    CsvGz,
    TsvGz,
    Tex,
    Bib,
    Bbl,
    Ini,
    Toml,
    Log,
    Rtf,
    Image,
    Unknown,
}

impl FileFormat {
    pub fn from_path(path: &Path) -> Self {
        let lower_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_lowercase());
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase());
        match ext.as_deref() {
            Some("pdf") => FileFormat::Pdf,
            Some("md") | Some("markdown") => FileFormat::Markdown,
            Some("txt") | Some("text") => FileFormat::Txt,
            Some("html") | Some("htm") => FileFormat::Html,
            Some("xml") | Some("xhtml") | Some("rss") | Some("atom") => FileFormat::Xml,
            Some("json") => FileFormat::Json,
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("csv") => FileFormat::Csv,
            Some("tsv") => FileFormat::Tsv,
            Some("tex") => FileFormat::Tex,
            Some("bib") => FileFormat::Bib,
            Some("bbl") => FileFormat::Bbl,
            Some("ini") | Some("cfg") | Some("conf") => FileFormat::Ini,
            Some("toml") => FileFormat::Toml,
            Some("log") => FileFormat::Log,
            Some("rtf") => FileFormat::Rtf,
            Some(ext)
                if matches!(
                    ext,
                    "png" | "jpg" | "jpeg" | "gif" | "tif" | "tiff" | "bmp" | "webp"
                ) =>
            {
                FileFormat::Image
            }
            _ => {
                if let Some(name) = lower_name {
                    if name.ends_with(".csv.gz") {
                        FileFormat::CsvGz
                    } else if name.ends_with(".tsv.gz") {
                        FileFormat::TsvGz
                    } else {
                        FileFormat::Unknown
                    }
                } else {
                    FileFormat::Unknown
                }
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FileFormat::Pdf => "pdf",
            FileFormat::Markdown => "md",
            FileFormat::Txt => "txt",
            FileFormat::Html => "html",
            FileFormat::Xml => "xml",
            FileFormat::Json => "json",
            FileFormat::Yaml => "yaml",
            FileFormat::Csv => "csv",
            FileFormat::Tsv => "tsv",
            FileFormat::CsvGz => "csv.gz",
            FileFormat::TsvGz => "tsv.gz",
            FileFormat::Tex => "tex",
            FileFormat::Bib => "bib",
            FileFormat::Bbl => "bbl",
            FileFormat::Ini => "ini",
            FileFormat::Toml => "toml",
            FileFormat::Log => "log",
            FileFormat::Rtf => "rtf",
            FileFormat::Image => "image",
            FileFormat::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawDocument {
    pub id: String,
    pub source_type: SourceType,
    pub format: FileFormat,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaSample {
    pub sample_id: String,
    pub task: String,
    pub doc_id: String,
    pub cell_ids: Vec<String>,
    pub question: String,
    pub answer: String,
    pub lang: String,
    pub meta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarySample {
    pub sample_id: String,
    pub task: String,
    pub doc_id: String,
    pub cell_ids: Vec<String>,
    pub title: Option<String>,
    pub summary: String,
    pub lang: String,
    pub meta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagSample {
    pub sample_id: String,
    pub task: String,
    pub doc_id: String,
    pub cell_ids: Vec<String>,
    pub question: String,
    pub answer: String,
    pub context: String,
    pub lang: String,
    pub meta: Value,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaskMetricsEntry {
    pub samples: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaskMetricsReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qa: Option<TaskMetricsEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TaskMetricsEntry>,
}

#[derive(Debug, Default, Clone)]
pub struct DatasetIndex {
    pub documents: HashMap<String, DocumentRecord>,
    pub cells_by_doc: HashMap<String, Vec<IndexCellRecord>>,
    pub cells_by_id: HashMap<String, IndexCellRecord>,
}

impl DatasetIndex {
    pub fn load(dataset_root: &Path) -> Result<Self> {
        let index_dir = dataset_root.join("index");
        let documents_path = index_dir.join("documents.jsonl");
        let cells_path = index_dir.join("cells.jsonl");
        if !documents_path.exists() || !cells_path.exists() {
            return Err(anyhow!("missing index files under {}", index_dir.display()));
        }
        let documents: Vec<DocumentRecord> = read_jsonl(&documents_path)?;
        let mut doc_map = HashMap::new();
        for doc in documents {
            doc_map.insert(doc.doc_id.clone(), doc);
        }
        let file = File::open(&cells_path)
            .with_context(|| format!("failed to open {}", cells_path.display()))?;
        let reader = BufReader::new(file);
        let mut cells_by_doc: HashMap<String, Vec<IndexCellRecord>> = HashMap::new();
        let mut cells_by_id = HashMap::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: IndexCellRecord =
                serde_json::from_str(&line).context("invalid cells.jsonl entry")?;
            cells_by_id.insert(record.cell_id.clone(), record.clone());
            cells_by_doc
                .entry(record.doc_id.clone())
                .or_default()
                .push(record);
        }
        for cells in cells_by_doc.values_mut() {
            cells.sort_by(|a, b| a.cell_id.cmp(&b.cell_id));
        }
        Ok(Self {
            documents: doc_map,
            cells_by_doc,
            cells_by_id,
        })
    }

    pub fn cells_for(&self, doc_id: &str) -> Option<&[IndexCellRecord]> {
        self.cells_by_doc
            .get(doc_id)
            .map(|records| records.as_slice())
    }

    pub fn lookup_cell(&self, cell_id: &str) -> Option<&IndexCellRecord> {
        self.cells_by_id.get(cell_id)
    }
}

pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: T = serde_json::from_str(&line).context("invalid jsonl entry")?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_jsonl_skips_blank_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample.jsonl");
        std::fs::write(&path, "{\"key\":1}\n\n{\"key\":2}\n").unwrap();
        let records: Vec<Value> = read_jsonl(&path).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].get("key").unwrap().as_i64().unwrap(), 1);
    }

    #[test]
    fn dataset_index_groups_cells_by_doc() {
        let dir = tempdir().unwrap();
        let index_dir = dir.path().join("index");
        std::fs::create_dir_all(&index_dir).unwrap();
        let docs = DocumentRecord {
            doc_id: "doc_1".to_string(),
            title: Some("Title".to_string()),
            source_type: "files".to_string(),
            source_format: "pdf".to_string(),
            source_ref: "/tmp/file.pdf".to_string(),
            tags: vec![],
        };
        let doc_line = serde_json::to_string(&docs).unwrap() + "\n";
        std::fs::write(index_dir.join("documents.jsonl"), doc_line).unwrap();
        let cell = IndexCellRecord {
            cell_id: "doc_1_cell_0001".to_string(),
            doc_id: "doc_1".to_string(),
            page_id: "doc_1_page_0001".to_string(),
            kind: "text".to_string(),
            text: "hello".to_string(),
            importance: 0.9,
            bbox: None,
            numguard: None,
            meta: Value::Null,
        };
        let cell_line = serde_json::to_string(&cell).unwrap() + "\n";
        std::fs::write(index_dir.join("cells.jsonl"), cell_line).unwrap();

        let dataset = DatasetIndex::load(dir.path()).unwrap();
        assert_eq!(dataset.documents.len(), 1);
        let cells = dataset.cells_for("doc_1").unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].text, "hello");
        assert!(dataset.lookup_cell("doc_1_cell_0001").is_some());
    }
}
