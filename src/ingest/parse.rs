use anyhow::{Context, Result};
use std::path::Path;

/// Extracts plain text from a file based on its extension.
/// Supports .txt/.md (read as-is) and .pdf (text layer only, no OCR).
pub fn parse_file(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();

    match ext.as_str() {
        "pdf" => pdf_extract::extract_text(path)
            .with_context(|| format!("failed to extract text from PDF {path:?}")),
        _ => std::fs::read_to_string(path).with_context(|| format!("failed to read {path:?}")),
    }
}
