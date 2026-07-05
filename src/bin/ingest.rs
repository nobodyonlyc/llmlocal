use anyhow::Result;
use clap::{Parser, Subcommand};
use llmlocal::config::Config;
use llmlocal::embed::{Embedder, DENSE_DIM};
use llmlocal::ingest::{ingest_text, parse_file};
use llmlocal::llm::LlmClient;
use llmlocal::state::AppState;
use llmlocal::store::Store;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse, chunk, embed, and upsert a file into the vector store.
    Ingest { path: PathBuf },
    /// Embed a query and print the top-k matching chunks.
    Query {
        text: String,
        #[arg(long, default_value_t = 5)]
        top_k: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    let config = Config::from_env();

    let store = Store::connect(&config.qdrant_url)?;
    store.ensure_collection(DENSE_DIM).await?;

    println!("loading BGE-M3 embedder (first run downloads the model)...");
    let embedder = Embedder::new()?;
    let llm = LlmClient::new(config.llama_server_url.clone());
    let state = AppState {
        config,
        embedder,
        store,
        llm,
    };

    match cli.command {
        Command::Ingest { path } => {
            let text = parse_file(&path)?;
            // Normalize to the basename so a document ingested via the CLI and
            // one uploaded via `/v1/ingest` (which only ever sees a filename,
            // not a filesystem path) resolve to the same source identity and
            // upsert rather than duplicate.
            let source = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            println!("parsed {} chars from {source}", text.len());
            let n = ingest_text(&state, &source, &text).await?;
            println!("embedded and upserted {n} chunks into collection 'docs'");
        }
        Command::Query { text, top_k } => {
            let vector = state.embedder.embed_one(&text)?;
            let hits = state.store.search(vector, top_k).await?;

            if hits.is_empty() {
                println!("no results (is the collection empty? run `ingest` first)");
            }
            for (i, hit) in hits.iter().enumerate() {
                println!(
                    "#{} score={:.4} source={}\n{}\n",
                    i + 1,
                    hit.score,
                    hit.source,
                    hit.text
                );
            }
        }
    }

    Ok(())
}
