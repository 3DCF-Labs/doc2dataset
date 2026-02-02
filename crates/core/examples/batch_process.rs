//! Example of batch processing multiple documents.
//!
//! Run with: `cargo run --example batch_process -- /path/to/docs/`

use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use three_dcf_core::index::CellRecord as IndexCellRecord;
use three_dcf_core::prelude::*;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    let input_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./documents"));

    if !input_dir.is_dir() {
        eprintln!("Directory not found: {}", input_dir.display());
        eprintln!("Usage: cargo run --example batch_process -- <directory>");
        std::process::exit(1);
    }

    // Collect supported files
    let files = collect_documents(&input_dir)?;
    println!("Found {} documents to process", files.len());

    if files.is_empty() {
        println!("No supported files found (.pdf, .md, .html, .txt)");
        return Ok(());
    }

    // Create output directory
    let output_dir = input_dir.join("output");
    fs::create_dir_all(&output_dir)?;

    // Create JSONL writers
    let docs_file = BufWriter::new(File::create(output_dir.join("documents.jsonl"))?);
    let pages_file = BufWriter::new(File::create(output_dir.join("pages.jsonl"))?);
    let cells_file = BufWriter::new(File::create(output_dir.join("cells.jsonl"))?);

    let mut docs_writer = JsonlWriter::new(docs_file);
    let mut pages_writer = JsonlWriter::new(pages_file);
    let mut cells_writer = JsonlWriter::new(cells_file);

    // Create encoder
    let encoder = Encoder::from_preset("reports")?;
    let serializer = TextSerializer::new();

    // Process each file
    let mut total_pages = 0u32;
    let mut total_cells = 0u32;

    for (idx, path) in files.iter().enumerate() {
        let doc_id = format!("doc_{:06}", idx);
        let filename = path.file_name().unwrap().to_string_lossy();

        print!("[{}/{}] Processing {}...", idx + 1, files.len(), filename);

        match encoder.encode_path(path) {
            Ok((document, metrics)) => {
                println!(" {} pages, {} cells", metrics.pages, metrics.cells_kept);

                // Write document record
                docs_writer.write_record(&DocumentRecord {
                    doc_id: doc_id.clone(),
                    title: Some(filename.to_string()),
                    source_type: "files".to_string(),
                    source_format: get_format(path),
                    source_ref: path.to_string_lossy().to_string(),
                    tags: vec![],
                })?;

                // Write page records
                for (page_idx, page_info) in document.pages.iter().enumerate() {
                    let page_id = format!("{}_page_{}", doc_id, page_idx);
                    pages_writer.write_record(&PageRecord {
                        page_id: page_id.clone(),
                        doc_id: doc_id.clone(),
                        page_number: page_info.z,
                        approx_tokens: None,
                        meta: serde_json::Value::Null,
                    })?;

                    // Write cell records for this page
                    for (cell_idx, cell) in document
                        .cells
                        .iter()
                        .filter(|c| c.z == page_info.z)
                        .enumerate()
                    {
                        let cell_id = format!("{}_cell_{}", page_id, cell_idx);
                        let text = document.payload_for(&cell.code_id).unwrap_or_default();

                        cells_writer.write_record(&IndexCellRecord {
                            cell_id,
                            doc_id: doc_id.clone(),
                            page_id: page_id.clone(),
                            kind: format!("{:?}", cell.cell_type).to_lowercase(),
                            text: text.to_string(),
                            importance: cell.importance as f32 / 100.0,
                            bbox: Some([
                                cell.x as f32,
                                cell.y as f32,
                                cell.w as f32,
                                cell.h as f32,
                            ]),
                            numguard: None,
                            meta: serde_json::Value::Null,
                        })?;
                    }
                }

                // Also write text serialization
                let text_output = serializer.to_string(&document)?;
                let output_path = output_dir.join(format!("{}.3dcf.txt", doc_id));
                fs::write(&output_path, text_output)?;

                total_pages += metrics.pages;
                total_cells += metrics.cells_kept;
            }
            Err(e) => {
                println!(" ERROR: {}", e);
            }
        }
    }

    // Flush writers
    docs_writer.flush()?;
    pages_writer.flush()?;
    cells_writer.flush()?;

    println!("\n=== Summary ===");
    println!("Documents: {}", files.len());
    println!("Pages:     {}", total_pages);
    println!("Cells:     {}", total_cells);
    println!("Output:    {}", output_dir.display());

    Ok(())
}

fn collect_documents(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let extensions = ["pdf", "md", "markdown", "html", "htm", "txt"];

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

fn get_format(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}
