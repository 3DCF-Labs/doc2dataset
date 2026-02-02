use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use three_dcf_core::index::JsonlWriter;

use crate::model::{DatasetIndex, QaSample};

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct OpenAiRecord {
    messages: Vec<OpenAiMessage>,
}

pub fn export(root: &Path, index: &DatasetIndex, qa: &[QaSample]) -> Result<()> {
    let dir = root.join("exports/openai");
    fs::create_dir_all(&dir)?;
    let file = File::create(dir.join("finetune.jsonl"))?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let messages = vec![
            OpenAiMessage {
                role: "system".to_string(),
                content: "You are an assistant and must answer strictly from the provided document context.".to_string(),
            },
            OpenAiMessage {
                role: "user".to_string(),
                content: format!("Context:\n{}\n\nQuestion: {}", context, sample.question),
            },
            OpenAiMessage {
                role: "assistant".to_string(),
                content: sample.answer.clone(),
            },
        ];
        writer.write_record(&OpenAiRecord { messages })?;
    }
    Ok(())
}

fn compose_context(cell_ids: &[String], index: &DatasetIndex) -> String {
    let mut parts = Vec::new();
    for cell_id in cell_ids {
        if let Some(cell) = index.lookup_cell(cell_id) {
            if !cell.text.trim().is_empty() {
                parts.push(cell.text.trim().to_string());
            }
        }
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DatasetIndex, QaSample};
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
            text: "Ctx".to_string(),
            importance: 0.5,
            bbox: None,
            numguard: None,
            meta: json!({}),
        };
        index.documents.insert("doc".to_string(), doc);
        index.cells_by_id.insert("cell".to_string(), cell.clone());
        index.cells_by_doc.insert("doc".to_string(), vec![cell]);
        index
    }

    #[test]
    fn exports_openai_records() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        let qa = vec![QaSample {
            sample_id: "qa".to_string(),
            task: "qa".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }];
        export(dir.path(), &index, &qa).unwrap();
        let contents =
            std::fs::read_to_string(dir.path().join("exports/openai/finetune.jsonl")).unwrap();
        let record: OpenAiRecord = serde_json::from_str(contents.lines().next().unwrap()).unwrap();
        assert_eq!(record.messages.len(), 3);
        assert!(record.messages[1].content.contains("Context"));
    }
}
