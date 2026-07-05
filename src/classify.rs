use crate::llm::{ChatMessage, JsonSchemaFormat};
use crate::state::AppState;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct ClassifyRequest {
    pub text: String,
    pub labels: Vec<String>,
}

#[derive(Serialize)]
pub struct ClassifyResponse {
    pub label: String,
}

#[derive(Deserialize)]
struct ModelOutput {
    label: String,
}

pub async fn classify(state: &AppState, req: ClassifyRequest) -> Result<ClassifyResponse> {
    let schema = json!({
        "type": "object",
        "properties": { "label": { "enum": req.labels } },
        "required": ["label"],
    });

    let system_prompt = format!(
        "You are a text classifier. Assign exactly one label from the allowed set to the \
         given text. Allowed labels: {}",
        req.labels.join(", ")
    );

    let content = state
        .llm
        .chat_completion(
            vec![
                ChatMessage::system(system_prompt),
                ChatMessage::user(format!("{} /no_think", req.text)),
            ],
            Some(JsonSchemaFormat {
                name: "classification",
                schema,
            }),
        )
        .await?;

    let parsed: ModelOutput =
        serde_json::from_str(&content).context("model output did not match classification schema")?;

    Ok(ClassifyResponse {
        label: parsed.label,
    })
}
