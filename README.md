# llmlocal

Local-first backend for RAG, classification, extraction, and routing over open-weight LLMs.
See `SPEC.md` for the architecture and rationale.

## Stack

- **API/orchestration**: Rust (axum + tokio)
- **Inference**: `llama-server` (llama.cpp), native host process, Vulkan GPU backend
- **Model**: Qwen3-8B-Instruct, GGUF Q4_K_M
- **Embeddings**: BAAI/bge-m3, in-process via `fastembed-rs` (CPU-only)
- **Vector store**: Qdrant, via podman-compose

## Setup

1. Download the model and inference binary:
   ```
   ./scripts/download-models.sh
   ```
2. Start the dev stack (llama-server + Qdrant):
   ```
   ./scripts/dev-up.sh
   ```
3. Run the API server:
   ```
   cargo run --bin server
   ```
   Listens on `http://127.0.0.1:3000`. Config is read from environment variables
   (see `.env.example`); copy it to `.env` to override defaults.

## CLI

`cargo run --bin ingest -- ingest <path>` — parse, chunk, embed, and upsert a document.
`cargo run --bin ingest -- query "<text>" [--top-k N]` — embed a query and print top matches.

## API

- `GET /healthz` — process liveness.
- `GET /readyz` — checks llama-server and Qdrant are reachable (503 if not).
- `POST /v1/ingest` — multipart file upload, chunks + embeds + stores it.
- `POST /v1/rag/query` — `{"query": "..."}` → `{"answer": "...", "sources": [...]}`.
- `POST /v1/classify` — `{"text": "...", "labels": ["a", "b"]}` → `{"label": "a"}`.
- `POST /v1/extract` — `{"text": "...", "schema": {...}}` → JSON matching the schema.
- `POST /v1/route` — `{"text": "...", "labels": [...]?, "schema": {...}?}` → dispatches to
  the above based on an embedding fast path with an LLM function-calling fallback; returns
  `{"routed_to", "routing_method", "result"}`.

## Known limitations / follow-ups

Not yet built: auth, production deployment hardening (TLS, rate limiting, resource limits),
formal Vietnamese+English evaluation datasets, `bge-reranker-v2-m3` for retrieval quality,
a mistral.rs migration spike (see `SPEC.md` §3.2), and a vLLM swap if concurrency needs grow
beyond what a single `llama-server` process can serve.
