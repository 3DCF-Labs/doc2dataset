//! Basic example of encoding a PDF document.
//!
//! Run with: `cargo run --example encode_pdf -- path/to/document.pdf`

use std::env;
use std::path::PathBuf;
use three_dcf_core::prelude::*;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get input path from command line
    let args: Vec<String> = env::args().collect();
    let input_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("sample.pdf"));

    if !input_path.exists() {
        eprintln!("File not found: {}", input_path.display());
        eprintln!("Usage: cargo run --example encode_pdf -- <path/to/document.pdf>");
        std::process::exit(1);
    }

    println!("Encoding: {}", input_path.display());

    // Create encoder with "reports" preset
    let encoder = Encoder::from_preset("reports")?;

    // Encode the document
    let (document, metrics) = encoder.encode_path(&input_path)?;

    // Print metrics
    println!("\n=== Encoding Metrics ===");
    println!("Pages:       {}", metrics.pages);
    println!("Total cells: {}", metrics.cells_total);
    println!("Kept cells:  {}", metrics.cells_kept);
    println!("Dedup ratio: {:.2}x", metrics.dedup_ratio);
    println!("NumGuards:   {}", metrics.numguard_count);

    // Serialize to text format
    let serializer = TextSerializer::new();
    let output = serializer.to_string(&document)?;

    println!("\n=== Serialized Output ===");
    // Print first 2000 chars
    if output.len() > 2000 {
        println!("{}...\n[truncated]", &output[..2000]);
    } else {
        println!("{}", output);
    }

    // Optionally write to file
    let output_path = input_path.with_extension("3dcf.txt");
    serializer.write_textual(&document, &output_path)?;
    println!("\nWritten to: {}", output_path.display());

    Ok(())
}
