#[derive(Clone, Debug)]
pub struct Config {
    pub llama_server_url: String,
    pub qdrant_url: String,
    pub default_top_k: u64,
    pub chunk_size: usize,
    /// Minimum cosine similarity a retrieved chunk must clear to be considered
    /// relevant context; below this, RAG reports insufficient context instead
    /// of answering from noise.
    pub min_score: f32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            llama_server_url: env_or("LLAMA_SERVER_URL", "http://127.0.0.1:8080"),
            qdrant_url: env_or("QDRANT_URL", "http://127.0.0.1:6334"),
            default_top_k: env_or("DEFAULT_TOP_K", "5").parse().unwrap_or(5),
            chunk_size: env_or("CHUNK_SIZE", "800").parse().unwrap_or(800),
            min_score: env_or("MIN_SCORE", "0.45").parse().unwrap_or(0.45),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
