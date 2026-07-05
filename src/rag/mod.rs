use crate::llm::ChatMessage;
use crate::state::AppState;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct RagSource {
    pub text: String,
    pub source: String,
    pub score: f32,
}

#[derive(Serialize)]
pub struct RagResponse {
    pub answer: String,
    pub sources: Vec<RagSource>,
}

const INSUFFICIENT_CONTEXT_ANSWER: &str =
    "I don't have enough information in the knowledge base to answer that.";

pub async fn answer(state: &AppState, query: &str) -> Result<RagResponse> {
    let query_vector = state.embedder.embed_one(query)?;
    let hits = state
        .store
        .search(query_vector, state.config.default_top_k)
        .await?;

    let relevant: Vec<_> = hits
        .into_iter()
        .filter(|h| h.score >= state.config.min_score)
        .collect();

    if relevant.is_empty() {
        return Ok(RagResponse {
            answer: INSUFFICIENT_CONTEXT_ANSWER.to_string(),
            sources: vec![],
        });
    }

    let context = relevant
        .iter()
        .enumerate()
        .map(|(i, h)| format!("[{}] {}", i + 1, h.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = format!(
        "You are a helpful assistant. Answer the user's question using ONLY the context \
         below. If the context does not contain enough information to answer, say so \
         explicitly rather than guessing. Answer in the same language as the question.\n\n\
         Context:\n{context}"
    );

    let content = state
        .llm
        .chat_completion(
            vec![
                ChatMessage::system(system_prompt),
                ChatMessage::user(format!("{query} /no_think")),
            ],
            None,
        )
        .await?;

    let sources = relevant
        .into_iter()
        .map(|h| RagSource {
            text: h.text,
            source: h.source,
            score: h.score,
        })
        .collect();

    Ok(RagResponse {
        answer: content,
        sources,
    })
}
