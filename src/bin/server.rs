use anyhow::Result;
use llmlocal::config::Config;
use llmlocal::embed::Embedder;
use llmlocal::llm::LlmClient;
use llmlocal::state::AppState;
use llmlocal::store::Store;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    let host = config.server_host.clone();
    let port = config.server_port;
    let store = Store::connect(&config.qdrant_url)?;
    store.ensure_collection(llmlocal::embed::DENSE_DIM).await?;

    tracing::info!("loading BGE-M3 embedder...");
    let embedder = Embedder::new()?;
    let llm = LlmClient::new(config.llama_server_url.clone());

    let state = Arc::new(AppState {
        config,
        embedder,
        store,
        llm,
    });

    tracing::info!("warming router seed embeddings...");
    llmlocal::router::warm(&state)?;

    let app = llmlocal::api::build_router(state);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
