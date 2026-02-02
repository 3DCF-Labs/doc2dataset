use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use three_dcf_core::index::JsonlWriter;

use crate::model::{read_jsonl, RagSample};

#[derive(Serialize, Deserialize)]
struct RagRecord {
    pub id: String,
    pub doc_id: String,
    pub question: String,
    pub answer: String,
    pub context: String,
    pub lang: String,
}

pub fn export(root: &Path) -> Result<()> {
    let samples_path = root.join("samples/rag.jsonl");
    if !samples_path.exists() {
        anyhow::bail!(
            "RAG samples not found at {}. Did you run tasks with 'qa'?",
            samples_path.display()
        );
    }
    let rag_samples: Vec<RagSample> = read_jsonl(&samples_path)
        .with_context(|| format!("failed to read {}", samples_path.display()))?;
    let out_dir = root.join("exports/rag");
    fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("train.jsonl");
    let file = File::create(&out_path)
        .with_context(|| format!("failed to create {}", out_path.display()))?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for sample in rag_samples {
        let record = RagRecord {
            id: sample.sample_id,
            doc_id: sample.doc_id,
            question: sample.question,
            answer: sample.answer,
            context: sample.context,
            lang: sample.lang,
        };
        writer.write_record(&record)?;
    }
    eprintln!("[doc2dataset] RAG jsonl written to {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn export_creates_train_file() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let samples_dir = root.join("samples");
        fs::create_dir_all(&samples_dir).unwrap();
        let rag = RagSample {
            sample_id: "rag_1".to_string(),
            task: "rag".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            context: "Ctx".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        };
        let file = File::create(samples_dir.join("rag.jsonl")).unwrap();
        let mut writer = JsonlWriter::new(BufWriter::new(file));
        writer.write_record(&rag).unwrap();
        drop(writer);
        export(root).unwrap();
        let out = root.join("exports/rag/train.jsonl");
        assert!(out.exists());
        let contents = std::fs::read_to_string(out).unwrap();
        assert!(contents.contains("rag_1"));
    }
}
