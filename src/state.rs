use crate::config::Config;
use crate::embed::Embedder;
use crate::llm::LlmClient;
use crate::store::Store;

/// Shared application state, `Arc`'d into axum's `State` extractor.
pub struct AppState {
    pub config: Config,
    pub embedder: Embedder,
    pub store: Store,
    pub llm: LlmClient,
}
