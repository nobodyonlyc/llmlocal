//! llmlocal: local-first backend for RAG, classification, extraction, and routing.

pub mod api;
pub mod classify;
pub mod config;
pub mod embed;
pub mod extract;
pub mod ingest;
pub mod llm;
pub mod rag;
pub mod router;
pub mod state;
pub mod store;
