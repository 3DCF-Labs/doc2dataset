use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use three_dcf_core::index::JsonlWriter;

use crate::model::{DatasetIndex, QaSample, SummarySample};

#[derive(Serialize, Deserialize)]
struct HfRecord {
    pub id: String,
    pub task: String,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub summary: Option<String>,
    pub context: String,
    pub doc_id: String,
}

#[derive(Serialize, Deserialize)]
struct HfMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
struct HfChatRecord {
    pub id: String,
    pub task: String,
    pub doc_id: String,
    pub messages: Vec<HfMessage>,
}

pub fn export(
    root: &Path,
    index: &DatasetIndex,
    qa: &[QaSample],
    summary: &[SummarySample],
) -> Result<()> {
    let out_dir = root.join("exports/hf");
    fs::create_dir_all(&out_dir)?;
    let writer_path = out_dir.join("train.jsonl");
    let file = File::create(&writer_path).context("failed to create HF export")?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for sample in qa {
        let context = compose_context(&sample.cell_ids, index);
        let record = HfRecord {
            id: sample.sample_id.clone(),
            task: sample.task.clone(),
            question: Some(sample.question.clone()),
            answer: Some(sample.answer.clone()),
            summary: None,
            context,
            doc_id: sample.doc_id.clone(),
        };
        writer.write_record(&record)?;
    }
    for sample in summary {
        let context = compose_context(&sample.cell_ids, index);
        let record = HfRecord {
            id: sample.sample_id.clone(),
            task: sample.task.clone(),
            question: None,
            answer: None,
            summary: Some(sample.summary.clone()),
            context,
            doc_id: sample.doc_id.clone(),
        };
        writer.write_record(&record)?;
    }
    let chat_file =
        File::create(out_dir.join("train_chat.jsonl")).context("failed to create HF chat jsonl")?;
    let mut chat_writer = JsonlWriter::new(BufWriter::new(chat_file));
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
            HfMessage {
                role: "user".to_string(),
                content: user,
            },
            HfMessage {
                role: "assistant".to_string(),
                content: sample.answer.clone(),
            },
        ];
        let record = HfChatRecord {
            id: sample.sample_id.clone(),
            task: sample.task.clone(),
            doc_id: sample.doc_id.clone(),
            messages,
        };
        chat_writer.write_record(&record)?;
    }
    write_dataset_card(&out_dir, qa.len(), summary.len())?;
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

fn write_dataset_card(out_dir: &Path, qa_count: usize, summary_count: usize) -> Result<()> {
    let card_path = out_dir.join("dataset_card.md");
    let mut lines = Vec::new();
    lines.push("# 3DCF doc2dataset export".to_string());
    lines.push("".to_string());
    lines.push(format!("- QA samples: {}", qa_count));
    lines.push(format!("- Summary samples: {}", summary_count));
    fs::write(&card_path, lines.join("\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DatasetIndex, QaSample, SummarySample};
    use serde_json::{json, Value};
    use tempfile::tempdir;
    use three_dcf_core::index::{CellRecord as IndexCellRecord, DocumentRecord};

    fn sample_index() -> DatasetIndex {
        let mut index = DatasetIndex::default();
        let doc = DocumentRecord {
            doc_id: "doc_1".to_string(),
            title: Some("Doc".to_string()),
            source_type: "files".to_string(),
            source_format: "pdf".to_string(),
            source_ref: "doc.pdf".to_string(),
            tags: vec![],
        };
        let cell = IndexCellRecord {
            cell_id: "doc_1_cell_0001".to_string(),
            doc_id: "doc_1".to_string(),
            page_id: "doc_1_page_0001".to_string(),
            kind: "text".to_string(),
            text: "Context".to_string(),
            importance: 0.8,
            bbox: None,
            numguard: None,
            meta: Value::Null,
        };
        index.documents.insert(doc.doc_id.clone(), doc);
        index
            .cells_by_doc
            .insert("doc_1".to_string(), vec![cell.clone()]);
        index.cells_by_id.insert(cell.cell_id.clone(), cell);
        index
    }

    #[test]
    fn exports_hf_records() {
        let dir = tempdir().unwrap();
        let index = sample_index();
        let qa = vec![QaSample {
            sample_id: "qa_0001".to_string(),
            task: "qa".to_string(),
            doc_id: "doc_1".to_string(),
            cell_ids: vec!["doc_1_cell_0001".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }];
        let summary = vec![SummarySample {
            sample_id: "summary_0001".to_string(),
            task: "summary".to_string(),
            doc_id: "doc_1".to_string(),
            cell_ids: vec!["doc_1_cell_0001".to_string()],
            title: Some("Section".to_string()),
            summary: "Summary".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        }];
        export(dir.path(), &index, &qa, &summary).unwrap();
        let train_path = dir.path().join("exports/hf/train.jsonl");
        let contents = std::fs::read_to_string(&train_path).unwrap();
        let mut lines = contents.lines();
        let first: HfRecord = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(first.question.unwrap(), "Q");
        assert!(first.context.contains("Context"));
        let card = std::fs::read_to_string(dir.path().join("exports/hf/dataset_card.md")).unwrap();
        assert!(card.contains("QA samples: 1"));
        assert!(lines.next().is_some());
        let chat_contents =
            std::fs::read_to_string(dir.path().join("exports/hf/train_chat.jsonl")).unwrap();
        let chat: HfChatRecord =
            serde_json::from_str(chat_contents.lines().next().unwrap()).unwrap();
        assert_eq!(chat.messages.len(), 2);
    }
}
