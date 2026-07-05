pub mod examples;

use crate::classify::{self, ClassifyRequest};
use crate::extract::{self, ExtractRequest};
use crate::llm::ChatMessage;
use crate::rag;
use crate::state::AppState;
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Rag,
    Classify,
    Extract,
    DirectChat,
}

/// Similarity above which the embedding fast path commits to a route without
/// falling back to the LLM. Tuned by hand-testing (see Phase 6 verification);
/// revisit if fast-path routing misfires in practice.
const FAST_PATH_THRESHOLD: f32 = 0.55;

struct SeedEmbeddings {
    route: Route,
    vector: Vec<f32>,
}

static SEED_EMBEDDINGS: OnceLock<Vec<SeedEmbeddings>> = OnceLock::new();

/// Pre-computes the seed-example embeddings so the first real `/v1/route`
/// request doesn't pay for it — call once at server startup.
pub fn warm(state: &AppState) -> Result<()> {
    seed_embeddings(state)?;
    Ok(())
}

/// Embeds the seed example utterances once (first call) and caches the result
/// for the process lifetime — small fixed set, no need for a vector store.
fn seed_embeddings(state: &AppState) -> Result<&'static Vec<SeedEmbeddings>> {
    if let Some(cached) = SEED_EMBEDDINGS.get() {
        return Ok(cached);
    }
    let examples = examples::seed_examples();
    let texts: Vec<String> = examples.iter().map(|(_, text)| text.to_string()).collect();
    let vectors = state.embedder.embed_batch(&texts)?;
    let embedded = examples
        .into_iter()
        .zip(vectors)
        .map(|((route, _), vector)| SeedEmbeddings { route, vector })
        .collect();
    Ok(SEED_EMBEDDINGS.get_or_init(|| embedded))
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub struct RouteDecision {
    pub route: Route,
    pub method: &'static str,
}

/// Two-tier routing: embedding-similarity fast path first (no LLM call), LLM
/// function-calling fallback for anything below the confidence threshold.
pub async fn decide(state: &AppState, text: &str) -> Result<RouteDecision> {
    let query_vector = state.embedder.embed_one(text)?;
    let seeds = seed_embeddings(state)?;

    let best = seeds
        .iter()
        .map(|s| (s.route, cosine_similarity(&query_vector, &s.vector)))
        .max_by(|a, b| a.1.total_cmp(&b.1));

    if let Some((route, score)) = best {
        if score >= FAST_PATH_THRESHOLD {
            return Ok(RouteDecision {
                route,
                method: "fast_path",
            });
        }
    }

    let tools = json!([
        {"type":"function","function":{"name":"rag_query","description":"Answer using the internal knowledge base of ingested documents (company policies, internal docs, or anything previously ingested)."}},
        {"type":"function","function":{"name":"classify","description":"Classify text into one of a set of labels (e.g. sentiment, topic, priority)."}},
        {"type":"function","function":{"name":"extract","description":"Extract structured fields/data out of unstructured text into JSON."}},
        {"type":"function","function":{"name":"direct_chat","description":"General conversation, question answering, or anything not covered by the other tools."}},
    ]);
    let tool_name = state
        .llm
        .pick_tool(
            vec![ChatMessage::user(format!("{text} /no_think"))],
            tools,
        )
        .await?;

    let route = match tool_name.as_deref() {
        Some("rag_query") => Route::Rag,
        Some("classify") => Route::Classify,
        Some("extract") => Route::Extract,
        _ => Route::DirectChat,
    };

    Ok(RouteDecision {
        route,
        method: "llm_fallback",
    })
}

/// Dispatches to the chosen capability. `classify`/`extract` need caller-supplied
/// labels/schema that a bare `{text}` router request doesn't carry — if the
/// router picks one of those routes but the required parameter is missing, we
/// report the routing decision without executing it rather than guessing a
/// label set or schema on the caller's behalf.
pub async fn dispatch(
    state: &AppState,
    text: &str,
    labels: Option<Vec<String>>,
    schema: Option<Value>,
) -> Result<Value> {
    let decision = decide(state, text).await?;

    let result = match decision.route {
        Route::Rag => serde_json::to_value(rag::answer(state, text).await?)?,
        Route::DirectChat => {
            let content = state
                .llm
                .chat_completion(vec![ChatMessage::user(format!("{text} /no_think"))], None)
                .await?;
            json!({ "answer": content })
        }
        Route::Classify => match labels {
            Some(labels) => serde_json::to_value(
                classify::classify(
                    state,
                    ClassifyRequest {
                        text: text.to_string(),
                        labels,
                    },
                )
                .await?,
            )?,
            None => json!({ "note": "routed to classify, but no 'labels' were provided; call /v1/classify directly with a label set" }),
        },
        Route::Extract => match schema {
            Some(schema) => extract::extract(
                state,
                ExtractRequest {
                    text: text.to_string(),
                    schema,
                },
            )
            .await?,
            None => json!({ "note": "routed to extract, but no 'schema' was provided; call /v1/extract directly with a JSON Schema" }),
        },
    };

    Ok(json!({
        "routed_to": decision.route,
        "routing_method": decision.method,
        "result": result,
    }))
}

