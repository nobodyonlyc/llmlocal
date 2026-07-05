pub mod handlers;

use crate::state::AppState;
use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .route("/v1/ingest", post(handlers::ingest))
        .route("/v1/rag/query", post(handlers::rag_query))
        .route("/v1/classify", post(handlers::classify))
        .route("/v1/comments/classify", post(handlers::classify_comment))
        .route("/v1/extract", post(handlers::extract))
        .route("/v1/route", post(handlers::route))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
