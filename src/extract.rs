use crate::llm::{ChatMessage, JsonSchemaFormat};
use crate::state::AppState;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct ExtractRequest {
    pub text: String,
    pub schema: Value,
}

const SYSTEM_PROMPT: &str = "You are a data extraction engine. Extract structured data from \
     the given text according to the required JSON schema. Only use information present in \
     the text; use null/omit fields that aren't present, don't invent values.";

pub async fn extract(state: &AppState, req: ExtractRequest) -> Result<Value> {
    let content = state
        .llm
        .chat_completion(
            vec![
                ChatMessage::system(SYSTEM_PROMPT),
                ChatMessage::user(format!("{} /no_think", req.text)),
            ],
            Some(JsonSchemaFormat {
                name: "extraction",
                schema: req.schema.clone(),
            }),
        )
        .await?;

    let parsed: Value =
        serde_json::from_str(&content).context("model did not return valid JSON")?;

    // Defense-in-depth: the grammar constrains shape, but not every constraint
    // (numeric ranges, string formats) — validate explicitly and surface a
    // loud error if the grammar path wasn't as airtight as expected for this
    // schema, rather than silently returning data that violates it.
    let validator = jsonschema::validator_for(&req.schema)
        .context("caller-supplied schema is not a valid JSON Schema")?;
    if let Err(err) = validator.validate(&parsed) {
        bail!("model output failed schema validation despite grammar constraint: {err}");
    }

    Ok(parsed)
}
