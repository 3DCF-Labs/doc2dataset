//! Example of advanced encoder and chunker configuration.

use std::io::Write;
use three_dcf_core::prelude::*;

fn main() -> Result<()> {
    // Builder-style configuration with importance tuning
    let encoder = EncoderBuilder::new("reports")?
        .budget(Some(4096))
        .drop_footers(true)
        .dedup_window(5)
        .hyphenation(HyphenationMode::Merge)
        .table_tolerance(24)
        .importance_tuning(ImportanceTuning {
            heading_boost: 1.4,
            number_boost: 1.2,
            footer_penalty: 0.4,
            early_line_bonus: 1.1,
        })
        .build();

    println!(
        "Preset: {:?} ({}x{} px)",
        encoder.config().preset,
        encoder.config().page_width_px,
        encoder.config().page_height_px
    );

    // Serializer configuration
    let serializer = TextSerializer::with_config(TextSerializerConfig {
        include_header: true,
        include_grammar: false,
        max_preview_chars: 200,
        table_mode: TableMode::Csv,
        preset_label: Some("custom".to_string()),
        budget_label: Some("4096".to_string()),
    });

    // Chunking for RAG
    let chunk_config = ChunkConfig {
        mode: ChunkMode::Tokens,
        cells_per_chunk: 200,
        overlap_cells: 20,
        max_tokens: 512,
        overlap_tokens: 64,
    };
    let chunker = Chunker::new(chunk_config);

    println!(
        "Chunker configured with max_tokens={} and overlap_tokens={}",
        chunk_config.max_tokens, chunk_config.overlap_tokens
    );

    // Encode an in-memory Markdown document by using a temp file
    let markdown = r#"
# Document Title

This is a paragraph with some **bold** text.

## Section 1

- Item 1
- Item 2
- Item 3

| Column A | Column B |
|----------|----------|
| Value 1  | Value 2  |
"#;

    let mut tmp = tempfile::NamedTempFile::new()?;
    tmp.write_all(markdown.as_bytes())?;

    let (document, metrics) = encoder.encode_path(tmp.path())?;
    println!(
        "Encoded markdown: {} cells from {} pages",
        metrics.cells_kept, metrics.pages
    );

    let serialized = serializer.to_string(&document)?;
    println!("\nSerialized preview:\n{}", &serialized[..serialized.len().min(300)]);

    let chunks = chunker.chunk_document(&document, "markdown_doc");
    println!("Generated {} chunks", chunks.len());

    Ok(())
}
