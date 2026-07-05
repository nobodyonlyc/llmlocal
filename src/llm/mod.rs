use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Clone)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system",
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user",
            content: content.into(),
        }
    }
}

/// A JSON Schema to constrain the model's output via llama-server's
/// grammar-backed `response_format` (see SPEC.md and Phase 0's curl spike).
#[derive(Serialize)]
pub struct JsonSchemaFormat {
    pub name: &'static str,
    pub schema: Value,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Deserialize)]
struct ToolCall {
    function: ToolCallFunction,
}

#[derive(Deserialize)]
struct ToolCallFunction {
    name: String,
}

pub struct LlmClient {
    http: reqwest::Client,
    base_url: String,
}

impl LlmClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    pub async fn is_healthy(&self) -> bool {
        self.http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    /// Calls llama-server's OpenAI-compatible /v1/chat/completions endpoint.
    /// `/no_think` is appended server-side by callers that need immediate
    /// (non-reasoning) output — see Qwen3 thinking-mode note in the plan.
    pub async fn chat_completion(
        &self,
        messages: Vec<ChatMessage>,
        json_schema: Option<JsonSchemaFormat>,
    ) -> Result<String> {
        let mut body = serde_json::json!({
            "model": "qwen3-8b",
            "messages": messages,
        });

        if let Some(schema) = json_schema {
            body["response_format"] = serde_json::json!({
                "type": "json_schema",
                "json_schema": { "name": schema.name, "schema": schema.schema },
            });
        }

        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("llama-server returned {status}: {text}");
        }

        let parsed: ChatCompletionResponse = resp.json().await?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        Ok(content)
    }

    /// Calls /v1/chat/completions with OpenAI-compatible `tools`/function-calling
    /// and returns the name of the tool the model chose to invoke, if any.
    /// Used as the router's LLM-fallback tier (see Phase 0/6 curl spikes).
    pub async fn pick_tool(&self, messages: Vec<ChatMessage>, tools: Value) -> Result<Option<String>> {
        let body = serde_json::json!({
            "model": "qwen3-8b",
            "messages": messages,
            "tools": tools,
        });

        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("llama-server returned {status}: {text}");
        }

        let parsed: ChatCompletionResponse = resp.json().await?;
        let tool_name = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.tool_calls.into_iter().next())
            .map(|tc| tc.function.name);
        Ok(tool_name)
    }
}
