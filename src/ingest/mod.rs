pub mod chunk;
pub mod parse;

pub use chunk::chunk_text;
pub use parse::parse_file;

use crate::state::AppState;
use crate::store::Chunk;
use anyhow::Result;
use std::hash::{Hash, Hasher};

/// Chunks, embeds, and upserts raw text under `source` (used both by the CLI
/// ingest command and the `/v1/ingest` HTTP handler). Returns the number of
/// chunks ingested.
pub async fn ingest_text(state: &AppState, source: &str, text: &str) -> Result<usize> {
    let pieces = chunk_text(text, state.config.chunk_size);
    let vectors = state.embedder.embed_batch(&pieces)?;

    let chunks = pieces
        .into_iter()
        .zip(vectors)
        .enumerate()
        .map(|(i, (text, vector))| Chunk {
            id: point_id(source, i),
            text,
            source: source.to_string(),
            vector,
        })
        .collect::<Vec<_>>();

    let n = chunks.len();
    state.store.upsert_chunks(chunks).await?;
    Ok(n)
}

fn point_id(source: &str, index: usize) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (source, index).hash(&mut hasher);
    hasher.finish()
}
