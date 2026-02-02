use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;

use crate::index::{CellRecord as IndexCellRecord, DocumentRecord, JsonlWriter, PageRecord};

use crate::{document::CellType, Document, Encoder};

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub preset: String,
    pub enable_ocr: bool,
    pub force_ocr: bool,
    pub ocr_languages: Vec<String>,
    pub source_override: Option<PathBuf>,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            preset: "reports".to_string(),
            enable_ocr: false,
            force_ocr: false,
            ocr_languages: vec!["eng".to_string()],
            source_override: None,
        }
    }
}

pub fn ingest_to_index(input_path: &Path, output_dir: &Path) -> Result<()> {
    let opts = IngestOptions::default();
    ingest_to_index_with_opts(input_path, output_dir, &opts)
}

pub fn ingest_to_index_with_opts(
    input_path: &Path,
    output_dir: &Path,
    opts: &IngestOptions,
) -> Result<()> {
    fs::create_dir_all(output_dir.join("index"))?;
    fs::create_dir_all(output_dir.join("raw/3dcf"))?;

    let ocr_langs = if opts.ocr_languages.is_empty() {
        vec!["eng".to_string()]
    } else {
        opts.ocr_languages.clone()
    };
    let builder = Encoder::builder(&opts.preset)?
        .enable_ocr(opts.enable_ocr)
        .force_ocr(opts.force_ocr)
        .ocr_languages(ocr_langs);
    let encoder = builder.build();
    let (document, _metrics) = encoder.encode_path(input_path)?;
    let source_path = opts.source_override.as_deref().unwrap_or(input_path);

    let doc_id = next_doc_id(&output_dir.join("raw/3dcf"))?;

    write_raw(&document, output_dir, &doc_id)?;
    write_index_records(output_dir, &doc_id, source_path, &document)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::tempdir;

    #[test]
    fn ingest_creates_raw_and_index_files() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("sample.md");
        std::fs::write(
            &input,
            "## Heading\n\n".to_string() + &"Body text ".repeat(50),
        )
        .unwrap();
        let output_dir = dir.path().join("dataset");

        ingest_to_index(&input, &output_dir).unwrap();

        assert!(output_dir.join("raw/3dcf/doc_0001.3dcf").exists());
        assert!(output_dir.join("raw/3dcf/doc_0001.3dcf.json").exists());

        let docs_path = output_dir.join("index/documents.jsonl");
        let docs_content = std::fs::read_to_string(&docs_path).unwrap();
        let first_line = docs_content.lines().next().unwrap();
        let doc_record: DocumentRecord = serde_json::from_str(first_line).unwrap();
        assert_eq!(doc_record.doc_id, "doc_0001");
        assert_eq!(doc_record.source_format, "md");

        let cells_path = output_dir.join("index/cells.jsonl");
        let cells_content = std::fs::read_to_string(cells_path).unwrap();
        assert!(!cells_content.is_empty());
        let first_cell: Value =
            serde_json::from_str(cells_content.lines().next().unwrap()).unwrap();
        assert_eq!(
            first_cell.get("doc_id").unwrap().as_str().unwrap(),
            "doc_0001"
        );
    }
}

fn write_raw(document: &Document, output_dir: &Path, doc_id: &str) -> Result<()> {
    let raw_dir = output_dir.join("raw/3dcf");
    fs::create_dir_all(&raw_dir)?;
    let bin_path = raw_dir.join(format!("{doc_id}.3dcf"));
    let json_path = raw_dir.join(format!("{doc_id}.3dcf.json"));
    document.save_bin(&bin_path)?;
    document.save_json(&json_path)?;
    Ok(())
}

fn write_index_records(
    output_dir: &Path,
    doc_id: &str,
    source_path: &Path,
    document: &Document,
) -> Result<()> {
    let index_dir = output_dir.join("index");
    fs::create_dir_all(&index_dir)?;
    let documents_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(index_dir.join("documents.jsonl"))?;
    let pages_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(index_dir.join("pages.jsonl"))?;
    let cells_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(index_dir.join("cells.jsonl"))?;

    let mut documents_writer = JsonlWriter::new(BufWriter::new(documents_file));
    let mut pages_writer = JsonlWriter::new(BufWriter::new(pages_file));
    let mut cells_writer = JsonlWriter::new(BufWriter::new(cells_file));

    let doc_record = DocumentRecord {
        doc_id: doc_id.to_string(),
        title: source_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|s| s.to_string()),
        source_type: "files".to_string(),
        source_format: source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_lowercase(),
        source_ref: source_path.display().to_string(),
        tags: Vec::new(),
    };
    documents_writer.write_record(&doc_record)?;

    let mut page_lookup = HashMap::new();
    for (idx, page) in document.pages.iter().enumerate() {
        let page_id = format!("{doc_id}_page_{:04}", idx + 1);
        let page_text = document.decode_page_to_text(page.z);
        let approx_tokens = if page_text.trim().is_empty() {
            None
        } else {
            Some(page_text.split_whitespace().count() as u32)
        };
        let page_record = PageRecord {
            page_id: page_id.clone(),
            doc_id: doc_id.to_string(),
            page_number: (idx + 1) as u32,
            approx_tokens,
            meta: json!({
                "width_px": page.width_px,
                "height_px": page.height_px,
                "z": page.z,
            }),
        };
        pages_writer.write_record(&page_record)?;
        page_lookup.insert(page.z, page_id);
    }

    let ordered_cells = document.ordered_cells();
    for (idx, cell) in ordered_cells.iter().enumerate() {
        let cell_id = format!("{doc_id}_cell_{:06}", idx + 1);
        let text = document
            .payload_for(&cell.code_id)
            .map(|s| s.to_string())
            .unwrap_or_default();
        let page_id = page_lookup
            .get(&cell.z)
            .cloned()
            .unwrap_or_else(|| format!("{doc_id}_page_{:04}", cell.z + 1));
        let bbox = Some([
            cell.x as f32,
            cell.y as f32,
            (cell.x as f32) + cell.w as f32,
            (cell.y as f32) + cell.h as f32,
        ]);
        let record = IndexCellRecord {
            cell_id,
            doc_id: doc_id.to_string(),
            page_id,
            kind: normalize_kind(cell.cell_type),
            text,
            importance: (cell.importance as f32) / 255.0,
            bbox,
            numguard: None,
            meta: json!({
                "rle": cell.rle,
            }),
        };
        cells_writer.write_record(&record)?;
    }

    Ok(())
}

fn normalize_kind(cell_type: CellType) -> String {
    match cell_type {
        CellType::Text => "text",
        CellType::Table => "table",
        CellType::Figure => "figure",
        CellType::Footer => "footer",
        CellType::Header => "heading",
    }
    .to_string()
}

fn next_doc_id(raw_dir: &Path) -> Result<String> {
    fs::create_dir_all(raw_dir)?;
    let mut max_id = 0u32;
    for entry in fs::read_dir(raw_dir)? {
        let entry = entry?;
        if !entry.path().is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if let Some(stripped) = name.strip_prefix("doc_") {
                if let Some(number_part) = stripped.strip_suffix(".3dcf") {
                    if let Ok(num) = number_part.parse::<u32>() {
                        max_id = max_id.max(num);
                    }
                }
            }
        }
    }
    Ok(format!("doc_{:04}", max_id + 1))
}
