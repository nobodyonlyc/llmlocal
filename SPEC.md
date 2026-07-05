# llmlocal — Spec

A local-first backend service that serves four capabilities on top of open-weight LLMs running on local hardware: **RAG**, **classification**, **extraction**, and **routing**.

## 1. Goals & constraints

- **Deployment target**: backend service (not a single-user CLI) — other apps/services call it over the network.
- **Priorities (all three, in tension — see §3 for how the architecture addresses each)**: fast inference/throughput, a fast API/backend layer, and low end-to-end latency.
- **Languages served**: Vietnamese + English, as first-class citizens in model choice, embeddings, and eval — not English-only with translation bolted on.
- **App/orchestration language**: Rust.
- **Hardware this must run well on**: single NVIDIA RTX 3060, 12 GB VRAM, 32 GB system RAM, 16 CPU cores (CachyOS/Linux). Model and quantization choices are constrained by the 12 GB VRAM budget.

## 2. Capabilities

1. **RAG** — ingest documents, chunk, embed, store, retrieve, generate grounded answers.
2. **Classification** — assign a label (or set of labels) to input text against a task-defined label set.
3. **Extraction** — pull structured data (JSON matching a schema) out of unstructured text.
4. **Router** — given an incoming request, decide which capability/tool/model should handle it.

These four are not independent subsystems bolted together — the router sits in front and dispatches to RAG/classification/extraction, and classification/extraction are reused as building blocks inside the RAG pipeline itself (e.g., classifying query intent before retrieval, extracting structured filters from a query).

## 3. Architecture

```
                          ┌─────────────────────────────┐
        HTTP/gRPC ───────▶│   Rust API layer (axum)     │
                          │   - auth, request validation │
                          │   - router dispatch           │
                          └───────┬───────────┬──────────┘
                                  │           │
                     ┌────────────┘           └────────────┐
                     ▼                                      ▼
        ┌─────────────────────────┐            ┌─────────────────────────┐
        │  In-process embeddings   │            │  Inference sidecar       │
        │  (fastembed-rs / candle) │            │  (llama.cpp server,      │
        │  BAAI/bge-m3             │            │  OpenAI-compatible API)  │
        └───────────┬──────────────┘            │  Qwen3-8B-Instruct GGUF  │
                    │                            └─────────────────────────┘
                    ▼
        ┌─────────────────────────┐
        │  Qdrant (vector store)   │
        │  rust-client, gRPC/tonic │
        └─────────────────────────┘
```

### 3.1 API / orchestration layer — Rust, axum + tokio

Handles HTTP, request validation, auth, and orchestration of the pipeline below. This is the layer directly in your control for "fast backend" — async, low per-request overhead, and it's where the router's fast path lives so most requests never need to round-trip to the LLM at all.

### 3.2 Inference engine — llama.cpp server (`llama-server`) as a sidecar, recommended default

Run as a separate process, called from Rust over its OpenAI-compatible HTTP API (`reqwest`, no FFI needed). Reasons to put the model behind a mature, separate server rather than embedding inference directly in the Rust binary:

- **Maturity**: llama.cpp is the most battle-tested local-inference engine; on an RTX 3060 an 8B GGUF model at Q4_K_M runs at roughly ~40 tok/s, a 14B at ~23 tok/s.
- **GBNF / JSON-schema grammar constrained decoding is built in** — directly relevant to the extraction capability: the model is *forced* to emit valid JSON matching a schema, not just prompted to.
- **Decoupling**: because it speaks the OpenAI chat/completions API, the app layer doesn't change if you later swap the backend (e.g., move to vLLM for higher concurrent throughput on better hardware).

**Alternative considered — mistral.rs** (pure-Rust engine built on Candle, OpenAI-compatible server, supports GGUF/ISQ quantization, embeddings, and multimodal in one binary). This would make the *entire* stack Rust with zero C++ dependency, which fits the "everything in Rust" instinct — but its JSON-schema/grammar-constrained decoding support is less proven than llama.cpp's today. **Open question**: revisit once extraction reliability is validated against llama.cpp's grammar support — mistral.rs is the natural next step if it closes that gap, since it would let a single Rust binary serve chat + embeddings without a sidecar process at all.

**Alternative considered — vLLM**: dramatically higher continuous-batching throughput under concurrent load (benchmarks show ~793 tok/s vs. llama.cpp/Ollama's ~40 tok/s at peak on comparable hardware), but it's a Python service, heavier to operate, and its throughput advantage matters most at concurrency levels a single 12 GB card won't sustain anyway. **Kept as the upgrade path**: since both speak the OpenAI-compatible API, swapping the sidecar for vLLM later (e.g., on a bigger GPU or multi-GPU box) requires no change to the Rust orchestration layer.

### 3.3 Embeddings — in-process, not a sidecar

Use `fastembed-rs` (Candle-backed) to run **BAAI/bge-m3** directly inside the Rust process. BGE-M3 supports 100+ languages (including Vietnamese) and produces dense + sparse + multi-vector embeddings from a single model, enabling hybrid search without running two embedding models. Running this in-process (vs. another HTTP hop) removes a network round trip from every embed call — directly serves the low-latency goal, since embedding happens on both ingestion and on every query.

**Reranker (optional, add if RAG relevance needs a boost)**: BAAI/bge-reranker-v2-m3, same language coverage.

### 3.4 Vector store — Qdrant

Written in Rust itself, has an official `qdrant-client` crate (gRPC via `tonic`), and benchmarks show meaningfully lower memory use than Go-based alternatives at the same dataset size. Run as a local service (Docker) initially; **Qdrant Edge** (embedded, in-process, no network) is a documented future option if the network hop to Qdrant ever becomes the bottleneck.

### 3.5 Router — embedding-based semantic router, LLM fallback

Two-tier design, mirroring the pattern used by projects like vLLM Semantic Router:

1. **Fast path**: embed the incoming request with the same in-process BGE-M3 model, compare against a small set of labeled example utterances per route (RAG / classify / extract / direct-chat), dispatch on cosine-similarity threshold. No LLM call — this is what keeps routing itself off the inference engine's critical path.
2. **Fallback**: if similarity is below threshold (ambiguous request), fall back to an LLM call (function-calling / tool-selection style prompt against the same Qwen3-8B model) to decide the route.

This avoids standing up or training a dedicated router model up front; it can be replaced with a small fine-tuned classifier later if the embedding-similarity approach proves too coarse.

### 3.6 Classification & extraction — model choice, not separate models

Both ride on the same primary LLM rather than dedicated fine-tuned models to start:

- **Classification**: prompt + constrained decoding restricting output to the task's label set.
- **Extraction**: prompt + GBNF/JSON-schema grammar (via llama.cpp) guaranteeing schema-valid JSON output — not just "ask nicely and hope."

This is deliberately the simplest thing that works. **Open question**: if either capability needs to run at higher volume/lower latency than the 8B generative model can sustain, revisit with a small dedicated classifier (e.g., a fine-tuned encoder) — but don't build that until the zero-shot approach is measured and found wanting.

## 4. Model choices

| Role | Model | Why |
|---|---|---|
| Primary LLM (generation, classification, extraction, router fallback) | **Qwen3-8B-Instruct**, GGUF, Q4_K_M or Q5_K_M | Strong Vietnamese + English performance, instruction-following and function-calling support, fits comfortably in 12 GB VRAM alongside the embedding model and KV cache headroom (~40 tok/s on RTX 3060 at Q4). |
| Embeddings | **BAAI/bge-m3** | 100+ languages incl. Vietnamese, dense+sparse+multi-vector in one model, small enough (~0.5B params) to run in-process alongside the main LLM without contending meaningfully for VRAM. |
| Reranker (optional) | **BAAI/bge-reranker-v2-m3** | Same language coverage; add if retrieval relevance needs improvement. |

VRAM budget check: Qwen3-8B at Q4_K_M is roughly 5–6 GB; BGE-M3 is well under 1.5 GB. That leaves meaningful headroom on a 12 GB card for KV cache and modest concurrency.

## 5. Open questions (not yet decided — need your input before implementation)

- **Auth**: how do calling apps authenticate to this backend (API keys, mTLS, none for now since it's internal)?
- **Deployment**: bare process + systemd, Docker Compose (API + llama-server + Qdrant), or something else?
- **Data ingestion**: what document sources/formats feed the RAG pipeline (PDFs, web pages, internal docs, DBs)? This determines the chunking/parsing story, which isn't designed yet.
- **Concurrency target**: roughly how many concurrent requests must this sustain? Determines whether the vLLM upgrade path in §3.2 needs to happen sooner rather than later.
- **Evaluation**: what's the acceptance bar for classification/extraction accuracy and RAG answer quality in Vietnamese specifically? No eval set exists yet.
- **mistral.rs migration**: worth a spike to check its grammar/JSON-schema constrained decoding maturity before committing long-term to the llama.cpp sidecar (§3.2).

## Sources consulted

- [mistral.rs (GitHub)](https://github.com/EricLBuehler/mistral.rs)
- [Rust Ecosystem for AI & LLMs](https://hackmd.io/@Hamze/Hy5LiRV1gg)
- [Best Local LLMs for RTX 3060 12GB](https://knightli.com/en/2026/05/08/rtx-3060-local-llm-models/)
- [13 Local LLMs, One RTX 3060](https://www.tyolab.com/blog/2026/05/11-64gb-ram-12gb-vram-the-honest-local-llm-benchmark/)
- [RTX 3060 12GB Local LLM (2026)](https://modelfit.io/gpu/rtx-3060/)
- [Best Open Source LLM For Vietnamese In 2026](https://www.siliconflow.com/articles/en/best-open-source-LLM-for-Vietnamese)
- [Qdrant](https://qdrant.tech/) / [Qdrant rust-client (GitHub)](https://github.com/qdrant/rust-client)
- [LLM router architecture: best practices for 2026 (Redis)](https://redis.io/blog/llm-router-architecture-best-practices/)
- [vLLM Semantic Router](https://vllm-semantic-router.com/) / [Signal-Decision Driven Architecture (vLLM blog)](https://blog.vllm.ai/2025/11/19/signal-decision.html)
- [Rig — Build AI agents in Rust](https://rig.rs/)
- [The Best Open-Source Embedding Models in 2026 (BentoML)](https://www.bentoml.com/blog/a-guide-to-open-source-embedding-models)
- [Grammar and Structured Output — llama.cpp (DeepWiki)](https://deepwiki.com/ggml-org/llama.cpp/8.1-grammar-and-structured-output)
- [fastembed-rs (GitHub)](https://github.com/Anush008/fastembed-rs)
- [Building a RAG Web Service with Qdrant and Rust (Shuttle)](https://www.shuttle.dev/blog/2024/02/28/rag-llm-rust)
