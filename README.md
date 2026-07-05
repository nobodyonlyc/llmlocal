# llmlocal

Local-first backend for RAG, classification, extraction, and routing over open-weight LLMs.
See `SPEC.md` for the architecture and rationale.

## Stack

- **API/orchestration**: Rust (axum + tokio), containerized
- **Inference**: `llama-server` (llama.cpp), containerized — CUDA image if a GPU is
  usable, CPU image otherwise
- **Model**: Qwen3-8B-Instruct, GGUF Q4_K_M
- **Embeddings**: BAAI/bge-m3, in-process via `fastembed-rs` (CPU-only)
- **Vector store**: Qdrant, containerized

Everything runs in containers (podman-compose, or `docker compose` if you have
Docker instead) — nothing is installed or run natively on the host.

## Setup

```
./scripts/dev-up.sh
```

This detects whether an NVIDIA GPU is usable from containers and brings up the
whole stack (Qdrant, llama-server, API) accordingly:

- **GPU path**: requires the `nvidia-container-toolkit` package and a generated
  CDI spec. One-time setup:
  ```
  sudo pacman -S --needed nvidia-container-toolkit   # or your distro's equivalent
  sudo nvidia-ctk cdi generate --output=/etc/cdi/nvidia.yaml
  ```
  `scripts/detect-gpu.sh` checks for `nvidia-smi` plus that CDI spec; if both
  are present, `llama-server` runs on the `server-cuda` image with the model
  fully offloaded to VRAM.
- **CPU path**: used automatically if the above isn't set up — `llama-server`
  runs on the `server` (CPU) image instead. Slower, but correct.

The GGUF model (~4.7GB) downloads automatically into a named volume on first
run. The API listens on `http://127.0.0.1:3000` (override the host port with
`API_HOST_PORT`). Config is read from environment variables (see
`.env.example`); those defaults are for running the API natively during
development (`cargo run --bin server`) — the containerized `api` service gets
its config from `deploy/podman-compose.yml` directly.

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
