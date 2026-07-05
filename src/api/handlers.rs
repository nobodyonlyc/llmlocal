use crate::classify::{self, ClassifyRequest, ClassifyResponse};
use crate::comment_classify::{self, CommentClassifyRequest, CommentClassifyResponse};
use crate::extract::{self, ExtractRequest};
use crate::rag;
use crate::router;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

pub struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self.0, "request failed");
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub chunks_ingested: usize,
    pub source: String,
}

pub async fn ingest(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<IngestResponse>, ApiError> {
    let field = multipart
        .next_field()
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected a multipart 'file' field"))?;
    let source = field.file_name().unwrap_or("upload").to_string();
    let bytes = field.bytes().await?;
    let text = String::from_utf8_lossy(&bytes).into_owned();

    let chunks_ingested = crate::ingest::ingest_text(&state, &source, &text).await?;
    Ok(Json(IngestResponse {
        chunks_ingested,
        source,
    }))
}

#[derive(Deserialize)]
pub struct RagQueryRequest {
    pub query: String,
}

pub async fn rag_query(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RagQueryRequest>,
) -> Result<Json<rag::RagResponse>, ApiError> {
    let response = rag::answer(&state, &req.query).await?;
    Ok(Json(response))
}

pub async fn classify(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClassifyRequest>,
) -> Result<Json<ClassifyResponse>, ApiError> {
    let response = classify::classify(&state, req).await?;
    Ok(Json(response))
}

pub async fn classify_comment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommentClassifyRequest>,
) -> Result<Json<CommentClassifyResponse>, ApiError> {
    let response = comment_classify::classify_comment(&state, req).await?;
    Ok(Json(response))
}

pub async fn extract(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtractRequest>,
) -> Result<Json<Value>, ApiError> {
    let response = extract::extract(&state, req).await?;
    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct RouteRequest {
    pub text: String,
    pub labels: Option<Vec<String>>,
    pub schema: Option<Value>,
}

pub async fn route(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RouteRequest>,
) -> Result<Json<Value>, ApiError> {
    let response = router::dispatch(&state, &req.text, req.labels, req.schema).await?;
    Ok(Json(response))
}

/// Process-alive check: no downstream calls, just confirms the HTTP server itself is up.
pub async fn healthz() -> Json<Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Confirms the server can actually serve requests: llama-server and Qdrant
/// are reachable. The embedding model is warmed at startup (see bin/server.rs)
/// rather than lazily here, so a green /readyz means the first real request
/// won't eat a cold-load penalty.
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    let (llm_ok, store_ok) = tokio::join!(state.llm.is_healthy(), state.store.is_healthy());

    if llm_ok && store_ok {
        Json(serde_json::json!({ "status": "ok", "llama_server": true, "qdrant": true }))
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not ready",
                "llama_server": llm_ok,
                "qdrant": store_ok,
            })),
        )
            .into_response()
    }
}
