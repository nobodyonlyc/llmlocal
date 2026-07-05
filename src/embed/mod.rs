use anyhow::Result;
use fastembed::{Bgem3Embedding, Bgem3InitOptions, Bgem3Model};
use std::sync::Mutex;

/// Dense embedding dimensionality produced by BGE-M3 (used to size the Qdrant collection).
pub const DENSE_DIM: u64 = 1024;

/// In-process BGE-M3 embedder (CPU-only: fastembed only enables GPU execution
/// providers behind an opt-in `cuda`/`accelerate` feature, which we don't enable,
/// so all VRAM stays available for llama-server).
pub struct Embedder {
    // fastembed's `embed` takes `&mut self`; wrap in a Mutex so `Embedder` is `Sync`
    // and can be shared across axum handlers via `Arc`.
    model: Mutex<Bgem3Embedding>,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let model = Bgem3Embedding::try_new(Bgem3InitOptions::new(Bgem3Model::BGEM3Q))?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }

    /// Dense embeddings only (v1 uses cosine search over dense vectors; BGE-M3's
    /// sparse/ColBERT outputs are available on the same call if hybrid search is
    /// added later, see `embed_batch_full`).
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self.model.lock().expect("embedder mutex poisoned");
        let output = model.embed(texts, None)?;
        Ok(output.dense)
    }

    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.embed_batch(&[text.to_string()])?.remove(0))
    }
}
