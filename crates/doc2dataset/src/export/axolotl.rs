use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use three_dcf_core::index::JsonlWriter;

use crate::model::{DatasetIndex, QaSample};

#[derive(Serialize, Deserialize)]
struct AxolotlMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct AxolotlChatRecord {
    messages: Vec<AxolotlMessage>,
}

#[derive(Serialize, Deserialize)]
struct AxolotlTextRecord {
    text: String,
}

pub fn export_chat(root: &Path, index: &DatasetIndex, qa: &[QaSample]) -> Result<()> {
    let out_dir = root.join("exports/axolotl");
    fs::create_dir_all(&out_dir)?;
    let file = File::create(out_dir.join("chat.jsonl"))?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));

    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let user = format!(
            "Context:\n{}\n\nQuestion: {}",
            context.trim(),
            sample.question
        );
        let messages = vec![
            AxolotlMessage {
                role: "user".to_string(),
                content: user,
            },
            AxolotlMessage {
                role: "assistant".to_string(),
                content: sample.answer.clone(),
            },
        ];
        writer.write_record(&AxolotlChatRecord { messages })?;
    }
    Ok(())
}

pub fn export_text(root: &Path, index: &DatasetIndex, qa: &[QaSample]) -> Result<()> {
    let out_dir = root.join("exports/axolotl");
    fs::create_dir_all(&out_dir)?;
    let file = File::create(out_dir.join("text.jsonl"))?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));

    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let text = format!("Context:\n{}\n\nAnswer:\n{}", context.trim(), sample.answer);
        writer.write_record(&AxolotlTextRecord { text })?;
    }
    Ok(())
}

fn compose_context(cell_ids: &[String], index: &DatasetIndex) -> String {
    let mut parts = Vec::new();
    for cell_id in cell_ids {
        if let Some(cell) = index.lookup_cell(cell_id) {
            let text = cell.text.trim();
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DatasetIndex;
    use serde_json::json;
    use tempfile::tempdir;
    use three_dcf_core::index::{CellRecord as IndexCellRecord, DocumentRecord};

    fn sample_index() -> DatasetIndex {
        let mut index = DatasetIndex::default();
        let doc = DocumentRecord {
            doc_id: "doc".to_string(),
            title: None,
            source_type: "files".to_string(),
            source_format: "pdf".to_string(),
            source_ref: "doc.pdf".to_string(),
            tags: vec![],
        };
        let cell = IndexCellRecord {
            cell_id: "cell".to_string(),
            doc_id: "doc".to_string(),
            page_id: "page".to_string(),
            kind: "text".to_string(),
            text: "ctx".to_string(),
            importance: 1.0,
            bbox: None,
            numguard: None,
            meta: json!({}),
        };
        index.documents.insert("doc".to_string(), doc);
        index.cells_by_id.insert("cell".to_string(), cell.clone());
        index.cells_by_doc.insert("doc".to_string(), vec![cell]);
        index
    }

    fn sample_qa() -> Vec<QaSample> {
        vec![QaSample {
            sample_id: "qa_1".to_string(),
            task: "qa".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }]
    }

    #[test]
    fn export_chat_writes_records() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        export_chat(dir.path(), &index, &sample_qa()).unwrap();
        let contents =
            std::fs::read_to_string(dir.path().join("exports/axolotl/chat.jsonl")).unwrap();
        assert!(contents.contains("Question"));
    }

    #[test]
    fn export_text_writes_records() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        export_text(dir.path(), &index, &sample_qa()).unwrap();
        let contents =
            std::fs::read_to_string(dir.path().join("exports/axolotl/text.jsonl")).unwrap();
        assert!(contents.contains("Answer"));
    }
}
