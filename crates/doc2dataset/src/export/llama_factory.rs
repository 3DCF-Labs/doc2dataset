use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use three_dcf_core::index::JsonlWriter;

use crate::model::{DatasetIndex, QaSample};

#[derive(Serialize, Deserialize)]
struct AlpacaRecord {
    instruction: String,
    input: String,
    output: String,
}

#[derive(Serialize, Deserialize)]
struct SharegptMessage {
    from: String,
    value: String,
}

#[derive(Serialize, Deserialize)]
struct SharegptRecord {
    id: String,
    conversations: Vec<SharegptMessage>,
}

pub fn export(root: &Path, index: &DatasetIndex, qa: &[QaSample], format: &str) -> Result<()> {
    match format {
        "alpaca" => export_alpaca(root, index, qa),
        "sharegpt" | "chat" => export_sharegpt(root, index, qa),
        other => Err(anyhow!(format!("unsupported llama factory format {other}"))),
    }
}

fn export_alpaca(root: &Path, index: &DatasetIndex, qa: &[QaSample]) -> Result<()> {
    let dir = root.join("exports/llama_factory");
    fs::create_dir_all(&dir)?;
    let path = dir.join("alpaca.jsonl");
    let file = File::create(&path)?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let record = AlpacaRecord {
            instruction: "Answer the question using only the document context.".to_string(),
            input: format!("Context:\n{}\n\nQuestion: {}", context, sample.question),
            output: sample.answer.clone(),
        };
        writer.write_record(&record)?;
    }
    Ok(())
}

fn export_sharegpt(root: &Path, index: &DatasetIndex, qa: &[QaSample]) -> Result<()> {
    let dir = root.join("exports/llama_factory");
    fs::create_dir_all(&dir)?;
    let path = dir.join("sharegpt.jsonl");
    let file = File::create(&path)?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let human_value = format!(
            "Context:\n{}\n\nQuestion: {}",
            context.trim(),
            sample.question
        );
        let conversations = vec![
            SharegptMessage {
                from: "human".to_string(),
                value: human_value,
            },
            SharegptMessage {
                from: "gpt".to_string(),
                value: sample.answer.clone(),
            },
        ];
        let record = SharegptRecord {
            id: sample.sample_id.clone(),
            conversations,
        };
        writer.write_record(&record)?;
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
    fn exports_llama_factory_alpaca() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        let samples = vec![QaSample {
            sample_id: "qa_1".to_string(),
            task: "qa".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }];
        export(dir.path(), &index, &samples, "alpaca").unwrap();
        let contents =
            std::fs::read_to_string(dir.path().join("exports/llama_factory/alpaca.jsonl")).unwrap();
        let record: AlpacaRecord = serde_json::from_str(contents.lines().next().unwrap()).unwrap();
        assert!(record.input.contains("Context"));
        assert_eq!(record.output, "A");
    }

    #[test]
    fn exports_llama_factory_sharegpt() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        let samples = vec![QaSample {
            sample_id: "qa_1".to_string(),
            task: "qa".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }];
        export(dir.path(), &index, &samples, "sharegpt").unwrap();
        let contents =
            std::fs::read_to_string(dir.path().join("exports/llama_factory/sharegpt.jsonl"))
                .unwrap();
        let record: SharegptRecord =
            serde_json::from_str(contents.lines().next().unwrap()).unwrap();
        assert_eq!(record.conversations.len(), 2);
        assert!(record.conversations[0].value.contains("Context"));
    }
}
