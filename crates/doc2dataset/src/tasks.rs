use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde_json::json;
use three_dcf_core::index::{CellRecord as IndexCellRecord, JsonlWriter};
use three_dcf_llm::{LlmClient, LlmRequest, LlmResponse};
use tokio::runtime::Runtime;

use crate::config::Doc2DatasetConfig;
#[cfg(test)]
use crate::config::{DEFAULT_QA_MAX_PER_DOC, DEFAULT_SUMMARY_MAX_PER_DOC};
use crate::logging;
use crate::model::{
    read_jsonl, DatasetIndex, QaSample, RagSample, SummarySample, TaskMetricsEntry,
    TaskMetricsReport,
};

const QA_MIN_CHARS: usize = 80;
const QA_MAX_CONTEXT_CHARS: usize = 900;
const SUMMARY_MIN_CHARS: usize = 200;
const SUMMARY_MAX_CONTEXT_CHARS: usize = 12_000;
const LLM_MAX_RETRIES: u32 = 3;
const QA_SYSTEM_PROMPT: &str =
    "You are an analyst who must answer strictly from the provided context.";
const SUMMARY_SYSTEM_PROMPT: &str = "You write concise, factual summaries of corporate documents.";

#[derive(Debug, PartialEq, Eq, Hash)]
enum TaskKind {
    Qa,
    Summary,
}

pub fn run(dataset_root: String, tasks: String) -> Result<()> {
    let root = PathBuf::from(dataset_root);
    let requested = parse_tasks(&tasks)?;
    if requested.is_empty() {
        return Err(anyhow!("no tasks selected"));
    }
    let config = Doc2DatasetConfig::from_env()?;
    let llm_client = LlmClient::new(config.provider, config.model.clone())?;
    let runtime = Runtime::new().context("failed to start tokio runtime")?;
    let index = DatasetIndex::load(&root)?;
    fs::create_dir_all(root.join("samples"))?;
    let mut report = TaskMetricsReport::default();
    let llm_runner = |system: Option<&str>, user: &str| -> Result<LlmResponse> {
        runtime.block_on(llm_client.chat(&LlmRequest {
            system: system.map(|s| s.to_string()),
            user: user.to_string(),
        }))
    };
    if requested.contains(&TaskKind::Qa) {
        logging::stage(
            "qa",
            format!(
                "starting QA generation for {} documents",
                index.documents.len()
            ),
        );
        let metrics = generate_qa(&root, &index, &config, &llm_runner)?;
        report.qa = Some(metrics);
        logging::stage("rag", "building RAG view from QA samples".to_string());
        build_rag_samples(&root, &index)?;
    }
    if requested.contains(&TaskKind::Summary) {
        logging::stage(
            "summary",
            format!(
                "starting summary generation for {} documents",
                index.documents.len()
            ),
        );
        let metrics = generate_summary(&root, &index, &config, &llm_runner)?;
        report.summary = Some(metrics);
    }
    if report.qa.is_some() || report.summary.is_some() {
        merge_metrics(&root, &report)?;
    }
    Ok(())
}

fn parse_tasks(spec: &str) -> Result<HashSet<TaskKind>> {
    let mut kinds = HashSet::new();
    for raw in spec.split(',') {
        let trimmed = raw.trim().to_lowercase();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed.as_str() {
            "qa" => {
                kinds.insert(TaskKind::Qa);
            }
            "summary" => {
                kinds.insert(TaskKind::Summary);
            }
            other => return Err(anyhow!(format!("unknown task {other}"))),
        }
    }
    Ok(kinds)
}

fn generate_qa(
    root: &Path,
    index: &DatasetIndex,
    config: &Doc2DatasetConfig,
    invoke: &impl Fn(Option<&str>, &str) -> Result<LlmResponse>,
) -> Result<TaskMetricsEntry> {
    let qa_path = root.join("samples/qa.jsonl");
    let file = File::create(&qa_path).context("failed to create qa.jsonl")?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    let mut metrics = TaskMetricsEntry::default();
    let mut sample_counter = 0usize;
    let doc_ids = sorted_doc_ids(index);
    let total_docs = doc_ids.len();
    for (position, doc_id) in doc_ids.into_iter().enumerate() {
        let mut created = 0usize;
        let doc_label = doc_label(index, &doc_id);
        let doc_timer = Instant::now();
        logging::stage(
            "qa",
            format!("processing {doc_label} ({}/{})", position + 1, total_docs),
        );
        let Some(cells) = index.cells_for(&doc_id) else {
            continue;
        };
        let mut idx = 0usize;
        while idx < cells.len() && created < config.qa_max_per_doc {
            let cell = &cells[idx];
            if !is_textual_cell(&cell.kind) {
                idx += 1;
                continue;
            }
            let text = cell.text.trim();
            if text.len() < QA_MIN_CHARS {
                idx += 1;
                continue;
            }
            let mut context_text = text.to_string();
            let mut cell_ids = vec![cell.cell_id.clone()];
            if context_text.len() < QA_MAX_CONTEXT_CHARS && idx + 1 < cells.len() {
                let next = &cells[idx + 1];
                if is_textual_cell(&next.kind) && !next.text.trim().is_empty() {
                    context_text.push_str("\n\n");
                    context_text.push_str(next.text.trim());
                    cell_ids.push(next.cell_id.clone());
                }
            }
            logging::verbose(format!(
                "[qa] doc={} cells={} context_chars={}",
                doc_id,
                cell_ids.join(","),
                context_text.len()
            ));
            let prompt = format!(
                "Here is a fragment of a document:\n{}\n\nGenerate a helpful question and a precise answer following the format:\nQuestion: ...\nAnswer: ...\nLanguage: {}.",
                context_text,
                config.lang
            );
            let call_started = Instant::now();
            let response =
                call_llm_with_retry(invoke, Some(QA_SYSTEM_PROMPT), &prompt, &doc_id, "qa")?;
            throttle_llm(config.llm_delay_ms);
            if let Some((question, answer)) = parse_qa_response(&response.content) {
                sample_counter += 1;
                created += 1;
                metrics.samples += 1;
                metrics.prompt_tokens += response.prompt_tokens as u64;
                metrics.completion_tokens += response.completion_tokens as u64;
                let sample = QaSample {
                    sample_id: format!("qa_{sample_counter:06}"),
                    task: "qa".to_string(),
                    doc_id: doc_id.clone(),
                    cell_ids,
                    question,
                    answer,
                    lang: config.lang.clone(),
                    meta: json!({
                        "context_chars": context_text.len(),
                    }),
                };
                writer.write_record(&sample)?;
                logging::stage(
                    "qa",
                    format!(
                        "created sample qa_{sample_counter:06} for {doc_label} (prompt={} completion={} elapsed={:?})",
                        response.prompt_tokens,
                        response.completion_tokens,
                        call_started.elapsed()
                    ),
                );
            }
            idx += 1;
        }
        logging::stage(
            "qa",
            format!(
                "finished {doc_label}: {created} sample(s) in {:?}",
                doc_timer.elapsed()
            ),
        );
    }
    Ok(metrics)
}

fn generate_summary(
    root: &Path,
    index: &DatasetIndex,
    config: &Doc2DatasetConfig,
    invoke: &impl Fn(Option<&str>, &str) -> Result<LlmResponse>,
) -> Result<TaskMetricsEntry> {
    let summary_path = root.join("samples/summary.jsonl");
    let file = File::create(&summary_path).context("failed to create summary.jsonl")?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    let mut metrics = TaskMetricsEntry::default();
    let mut sample_counter = 0usize;
    let doc_ids = sorted_doc_ids(index);
    let total_docs = doc_ids.len();
    for (position, doc_id) in doc_ids.into_iter().enumerate() {
        let Some(cells) = index.cells_for(&doc_id) else {
            continue;
        };
        let sections = build_sections(cells);
        let mut created = 0usize;
        let doc_label = doc_label(index, &doc_id);
        let doc_timer = Instant::now();
        logging::stage(
            "summary",
            format!("processing {doc_label} ({}/{})", position + 1, total_docs),
        );
        for section in sections {
            if created >= config.summary_max_per_doc {
                break;
            }
            if section.text.chars().count() < SUMMARY_MIN_CHARS {
                continue;
            }
            let truncated = clamp_text(&section.text, SUMMARY_MAX_CONTEXT_CHARS);
            let prompt = format!(
                "Write a concise summary of the document section. Language: {}.\n\nHeading: {}\n\n{}",
                config.lang,
                section.title.as_deref().unwrap_or("(untitled)"),
                truncated
            );
            logging::verbose(format!(
                "[summary] doc={} chars={} heading={}",
                doc_id,
                section.text.len(),
                section.title.as_deref().unwrap_or("(untitled)")
            ));
            let call_started = Instant::now();
            let response = call_llm_with_retry(
                invoke,
                Some(SUMMARY_SYSTEM_PROMPT),
                &prompt,
                &doc_id,
                "summary",
            )?;
            throttle_llm(config.llm_delay_ms);
            let summary = response.content.trim();
            if summary.is_empty() {
                continue;
            }
            sample_counter += 1;
            created += 1;
            metrics.samples += 1;
            metrics.prompt_tokens += response.prompt_tokens as u64;
            metrics.completion_tokens += response.completion_tokens as u64;
            let sample = SummarySample {
                sample_id: format!("summary_{sample_counter:06}"),
                task: "summary".to_string(),
                doc_id: doc_id.clone(),
                cell_ids: section.cell_ids,
                title: section.title,
                summary: summary.to_string(),
                lang: config.lang.clone(),
                meta: json!({
                    "context_chars": section.text.chars().count(),
                    "truncated": section.text.chars().count() > SUMMARY_MAX_CONTEXT_CHARS,
                }),
            };
            writer.write_record(&sample)?;
            logging::stage(
                "summary",
                format!(
                    "created sample summary_{sample_counter:06} for {doc_label} (prompt={} completion={} elapsed={:?})",
                    response.prompt_tokens,
                    response.completion_tokens,
                    call_started.elapsed()
                ),
            );
        }
        logging::stage(
            "summary",
            format!(
                "finished {doc_label}: {created} summary sample(s) in {:?}",
                doc_timer.elapsed()
            ),
        );
    }
    Ok(metrics)
}

fn sorted_doc_ids(index: &DatasetIndex) -> Vec<String> {
    let mut doc_ids: Vec<String> = index.documents.keys().cloned().collect();
    doc_ids.sort();
    doc_ids
}

fn is_textual_cell(kind: &str) -> bool {
    matches!(kind, "text" | "heading")
}

fn parse_qa_response(raw: &str) -> Option<(String, String)> {
    let mut question = None;
    let mut answer = None;
    for line in raw.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("question") {
            question = Some(line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string());
        } else if lower.starts_with("answer") {
            answer = Some(line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string());
        }
    }
    if question.as_ref().map(|s| s.is_empty()).unwrap_or(true)
        || answer.as_ref().map(|s| s.is_empty()).unwrap_or(true)
    {
        let parts: Vec<&str> = raw.splitn(2, '\n').collect();
        if parts.len() == 2 {
            question = Some(parts[0].trim().to_string());
            answer = Some(parts[1].trim().to_string());
        }
    }
    match (question, answer) {
        (Some(q), Some(a)) if !q.is_empty() && !a.is_empty() => Some((q, a)),
        _ => None,
    }
}

struct Section {
    title: Option<String>,
    text: String,
    cell_ids: Vec<String>,
}

fn build_sections(cells: &[IndexCellRecord]) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = Section {
        title: None,
        text: String::new(),
        cell_ids: Vec::new(),
    };
    for cell in cells {
        if cell.kind == "heading" {
            if !current.text.trim().is_empty() {
                sections.push(current);
            }
            current = Section {
                title: Some(cell.text.trim().to_string()),
                text: String::new(),
                cell_ids: vec![cell.cell_id.clone()],
            };
            continue;
        }
        if cell.kind == "text" {
            if current.cell_ids.is_empty() {
                current.cell_ids.push(cell.cell_id.clone());
            } else {
                current.cell_ids.push(cell.cell_id.clone());
            }
            if !cell.text.trim().is_empty() {
                if !current.text.is_empty() {
                    current.text.push_str("\n\n");
                }
                current.text.push_str(cell.text.trim());
            }
        }
    }
    if !current.text.trim().is_empty() {
        sections.push(current);
    }
    sections
}

fn build_rag_samples(root: &Path, index: &DatasetIndex) -> Result<()> {
    let qa_samples: Vec<QaSample> = read_jsonl(&root.join("samples/qa.jsonl"))?;
    if qa_samples.is_empty() {
        return Ok(());
    }
    logging::stage(
        "rag",
        format!("creating {} rag samples from QA outputs", qa_samples.len()),
    );
    let rag_path = root.join("samples/rag.jsonl");
    let file = File::create(&rag_path)
        .with_context(|| format!("failed to create {}", rag_path.display()))?;
    let mut writer = JsonlWriter::new(BufWriter::new(file));
    for (i, sample) in qa_samples.iter().enumerate() {
        let context = compose_context(&sample.cell_ids, index);
        if context.trim().is_empty() {
            continue;
        }
        let record = RagSample {
            sample_id: format!("rag_{:06}", i + 1),
            task: "rag".to_string(),
            doc_id: sample.doc_id.clone(),
            cell_ids: sample.cell_ids.clone(),
            question: sample.question.clone(),
            answer: sample.answer.clone(),
            context,
            lang: sample.lang.clone(),
            meta: sample.meta.clone(),
        };
        writer.write_record(&record)?;
    }
    Ok(())
}

fn doc_label(index: &DatasetIndex, doc_id: &str) -> String {
    if let Some(doc) = index.documents.get(doc_id) {
        if let Some(title) = &doc.title {
            if title.trim().is_empty() {
                return doc_id.to_string();
            }
            return format!("{} – {}", doc_id, title.trim());
        }
    }
    doc_id.to_string()
}

fn throttle_llm(delay_ms: u64) {
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }
}

fn clamp_text(text: &str, max_chars: usize) -> String {
    let mut indices = text.char_indices();
    let mut end = text.len();
    for (i, (idx, _)) in indices.by_ref().enumerate() {
        if i >= max_chars {
            end = idx;
            break;
        }
    }
    if end >= text.len() {
        text.to_string()
    } else {
        format!("{}…", &text[..end])
    }
}

fn call_llm_with_retry(
    invoke: &impl Fn(Option<&str>, &str) -> Result<LlmResponse>,
    system: Option<&str>,
    prompt: &str,
    doc_id: &str,
    stage_name: &str,
) -> Result<LlmResponse> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match invoke(system, prompt) {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                logging::stage(
                    stage_name,
                    format!(
                        "LLM call failed for {doc_id} (attempt {attempt}/{LLM_MAX_RETRIES}): {err}"
                    ),
                );
                if attempt >= LLM_MAX_RETRIES {
                    return Err(err);
                }
                thread::sleep(Duration::from_secs((attempt * 2) as u64));
            }
        }
    }
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

fn merge_metrics(root: &Path, report: &TaskMetricsReport) -> Result<()> {
    let metrics_dir = root.join("metrics");
    fs::create_dir_all(&metrics_dir)?;
    let path = metrics_dir.join("tasks.json");
    let mut current = if path.exists() {
        let file = File::open(&path)?;
        serde_json::from_reader(file).unwrap_or_default()
    } else {
        TaskMetricsReport::default()
    };
    if report.qa.is_some() {
        current.qa = report.qa.clone();
    }
    if report.summary.is_some() {
        current.summary = report.summary.clone();
    }
    let file = File::create(&path)?;
    serde_json::to_writer_pretty(file, &current)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Doc2DatasetConfig;
    use crate::model::{QaSample, RagSample, SummarySample};
    use serde_json::json;
    use tempfile::tempdir;
    use three_dcf_core::index::{CellRecord as IndexCellRecord, DocumentRecord};
    use three_dcf_llm::{LlmProvider, LlmResponse};

    #[test]
    fn parse_tasks_handles_duplicates() {
        let parsed = parse_tasks("qa, summary, qa").unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn parse_qa_response_handles_keywords_and_fallback() {
        let raw = "Question: What?\nAnswer: Because";
        let parsed = parse_qa_response(raw).unwrap();
        assert_eq!(parsed.0, "What?");
        assert_eq!(parsed.1, "Because");

        let fallback = parse_qa_response("Why?\nBecause").unwrap();
        assert_eq!(fallback.0, "Why?");
    }

    #[test]
    fn build_sections_groups_by_heading() {
        let cells = vec![
            IndexCellRecord {
                cell_id: "1".to_string(),
                doc_id: "doc".to_string(),
                page_id: "p1".to_string(),
                kind: "heading".to_string(),
                text: "Intro".to_string(),
                importance: 0.9,
                bbox: None,
                numguard: None,
                meta: json!({}),
            },
            IndexCellRecord {
                cell_id: "2".to_string(),
                doc_id: "doc".to_string(),
                page_id: "p1".to_string(),
                kind: "text".to_string(),
                text: "Body".to_string(),
                importance: 0.5,
                bbox: None,
                numguard: None,
                meta: json!({}),
            },
        ];
        let sections = build_sections(&cells);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title.as_deref(), Some("Intro"));
        assert!(sections[0].cell_ids.contains(&"1".to_string()));
        assert!(sections[0].cell_ids.contains(&"2".to_string()));
    }

    #[test]
    fn generate_qa_writes_samples_with_stub_llm() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let index = sample_index(
            "doc",
            vec![long_cell("doc", "doc_cell_1", "Some long text".repeat(10))],
        );
        let config = Doc2DatasetConfig {
            provider: LlmProvider::OpenAi,
            model: "stub".into(),
            lang: "ru".into(),
            llm_delay_ms: 0,
            qa_max_per_doc: DEFAULT_QA_MAX_PER_DOC,
            summary_max_per_doc: DEFAULT_SUMMARY_MAX_PER_DOC,
        };
        std::fs::create_dir_all(root.join("samples")).unwrap();
        let response = LlmResponse {
            content: "Question: Q\nAnswer: A".to_string(),
            prompt_tokens: 10,
            completion_tokens: 5,
        };
        let metrics = generate_qa(root, &index, &config, &|_, _| Ok(response.clone())).unwrap();
        assert_eq!(metrics.samples, 1);
        assert_eq!(metrics.prompt_tokens, 10);
        let qa_path = root.join("samples/qa.jsonl");
        let contents = std::fs::read_to_string(qa_path).unwrap();
        let sample: QaSample = serde_json::from_str(contents.lines().next().unwrap()).unwrap();
        assert_eq!(sample.question, "Q");
        assert_eq!(sample.answer, "A");
    }

    #[test]
    fn generate_summary_writes_samples_with_stub_llm() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let cells = vec![
            heading_cell("doc", "doc_cell_0", "Intro"),
            long_cell("doc", "doc_cell_1", "Summary text ".repeat(20)),
        ];
        let index = sample_index("doc", cells);
        let config = Doc2DatasetConfig {
            provider: LlmProvider::OpenAi,
            model: "stub".into(),
            lang: "ru".into(),
            llm_delay_ms: 0,
            qa_max_per_doc: DEFAULT_QA_MAX_PER_DOC,
            summary_max_per_doc: DEFAULT_SUMMARY_MAX_PER_DOC,
        };
        std::fs::create_dir_all(root.join("samples")).unwrap();
        let response = LlmResponse {
            content: "Brief summary".to_string(),
            prompt_tokens: 8,
            completion_tokens: 4,
        };
        let metrics =
            generate_summary(root, &index, &config, &|_, _| Ok(response.clone())).unwrap();
        assert_eq!(metrics.samples, 1);
        let summary_path = root.join("samples/summary.jsonl");
        let contents = std::fs::read_to_string(summary_path).unwrap();
        let sample: SummarySample = serde_json::from_str(contents.lines().next().unwrap()).unwrap();
        assert_eq!(sample.summary, "Brief summary");
    }

    #[test]
    fn build_rag_samples_creates_file() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("samples")).unwrap();
        let index = sample_index(
            "doc",
            vec![long_cell("doc", "cell", "Context text".repeat(5))],
        );
        let qa = QaSample {
            sample_id: "qa_1".to_string(),
            task: "qa".to_string(),
            doc_id: "doc".to_string(),
            cell_ids: vec!["cell".to_string()],
            question: "Q".to_string(),
            answer: "A".to_string(),
            lang: "ru".to_string(),
            meta: json!({}),
        };
        let file = File::create(root.join("samples/qa.jsonl")).unwrap();
        let mut writer = JsonlWriter::new(BufWriter::new(file));
        writer.write_record(&qa).unwrap();
        drop(writer);
        build_rag_samples(root, &index).unwrap();
        let rag_contents = std::fs::read_to_string(root.join("samples/rag.jsonl")).unwrap();
        let rag: RagSample = serde_json::from_str(rag_contents.lines().next().unwrap()).unwrap();
        assert_eq!(rag.question, "Q");
        assert!(rag.context.contains("Context"));
    }

    fn sample_index(doc_id: &str, cells: Vec<IndexCellRecord>) -> DatasetIndex {
        let mut index = DatasetIndex::default();
        let doc = DocumentRecord {
            doc_id: doc_id.to_string(),
            title: None,
            source_type: "files".to_string(),
            source_format: "md".to_string(),
            source_ref: "doc.md".to_string(),
            tags: vec![],
        };
        index.documents.insert(doc_id.to_string(), doc);
        index.cells_by_id = cells
            .iter()
            .map(|cell| (cell.cell_id.clone(), cell.clone()))
            .collect();
        index.cells_by_doc.insert(doc_id.to_string(), cells);
        index
    }

    fn long_cell(doc: &str, id: &str, text: String) -> IndexCellRecord {
        IndexCellRecord {
            cell_id: id.to_string(),
            doc_id: doc.to_string(),
            page_id: "page".to_string(),
            kind: "text".to_string(),
            text,
            importance: 1.0,
            bbox: None,
            numguard: None,
            meta: json!({}),
        }
    }

    fn heading_cell(doc: &str, id: &str, text: &str) -> IndexCellRecord {
        IndexCellRecord {
            cell_id: id.to_string(),
            doc_id: doc.to_string(),
            page_id: "page".to_string(),
            kind: "heading".to_string(),
            text: text.to_string(),
            importance: 1.0,
            bbox: None,
            numguard: None,
            meta: json!({}),
        }
    }
}
