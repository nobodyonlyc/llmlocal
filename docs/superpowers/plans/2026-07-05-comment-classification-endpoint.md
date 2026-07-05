# Comment Classification Endpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated API endpoint for classifying story comments into structured quality signals for later story scoring.

**Architecture:** Keep the existing generic `/v1/classify` and `/v1/extract` endpoints unchanged. Add a focused `comment_classify` module with a fixed JSON schema, a Vietnamese-aware system prompt, validation, and one HTTP route at `POST /v1/comments/classify`. The first version classifies one comment per request; batch classification is intentionally left out until the single-comment contract is stable.

**Tech Stack:** Rust 2024, axum 0.8, tokio, serde, serde_json, jsonschema, reqwest, llama-server OpenAI-compatible chat completions with JSON schema response format.

---

## File Structure

- Create `src/comment_classify.rs`
  - Owns request/response types, fixed label enums, JSON schema construction, prompt construction, model output parsing, and validation.
- Modify `src/lib.rs`
  - Exposes the new module.
- Modify `src/api/handlers.rs`
  - Adds a thin handler that delegates to `comment_classify::classify_comment`.
- Modify `src/api/mod.rs`
  - Registers `POST /v1/comments/classify`.
- Modify `restapi/api.http`
  - Adds an example request for Vietnamese story comments.
- Modify `README.md`
  - Documents the new endpoint.
- Add tests inside `src/comment_classify.rs`
  - Unit tests cover schema shape, prompt content, deserialization, and validation helpers without requiring a live LLM.

## API Contract

Request:

```json
{
  "comment_id": "optional-source-comment-id",
  "story_id": "optional-story-id",
  "chapter_id": "optional-chapter-id",
  "text": "Truyen hay, main thong minh, mong ra tiep."
}
```

Response:

```json
{
  "comment_id": "optional-source-comment-id",
  "story_id": "optional-story-id",
  "chapter_id": "optional-chapter-id",
  "sentiment": "positive",
  "intent": "story_quality",
  "strength": 0.8,
  "confidence": 0.9,
  "is_quality_signal": true,
  "reason": "The comment praises the story and protagonist."
}
```

Allowed values:

```text
sentiment = positive | negative | neutral | mixed
intent = story_quality | translation_quality | update_request | spam | social | question
strength = number from 0.0 to 1.0
confidence = number from 0.0 to 1.0
is_quality_signal = true only when the comment is useful for story quality scoring
reason = short English diagnostic sentence, max 160 characters
```

---

### Task 1: Add Comment Classification Types, Schema, and Tests

**Files:**
- Create: `src/comment_classify.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/comment_classify.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_schema_contains_fixed_comment_labels() {
        let schema = comment_output_schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(
            schema["properties"]["sentiment"]["enum"],
            json!(["positive", "negative", "neutral", "mixed"])
        );
        assert_eq!(
            schema["properties"]["intent"]["enum"],
            json!([
                "story_quality",
                "translation_quality",
                "update_request",
                "spam",
                "social",
                "question"
            ])
        );
        assert_eq!(
            schema["required"],
            json!([
                "sentiment",
                "intent",
                "strength",
                "confidence",
                "is_quality_signal",
                "reason"
            ])
        );
    }

    #[test]
    fn model_output_deserializes_valid_payload() {
        let parsed: CommentModelOutput = serde_json::from_value(json!({
            "sentiment": "positive",
            "intent": "story_quality",
            "strength": 0.8,
            "confidence": 0.9,
            "is_quality_signal": true,
            "reason": "Praises the story quality."
        }))
        .unwrap();

        assert_eq!(parsed.sentiment, CommentSentiment::Positive);
        assert_eq!(parsed.intent, CommentIntent::StoryQuality);
        assert_eq!(parsed.strength, 0.8);
        assert_eq!(parsed.confidence, 0.9);
        assert!(parsed.is_quality_signal);
    }

    #[test]
    fn prompt_mentions_vietnamese_story_comment_policy() {
        let prompt = system_prompt();

        assert!(prompt.contains("Vietnamese"));
        assert!(prompt.contains("story_quality"));
        assert!(prompt.contains("translation_quality"));
        assert!(prompt.contains("update_request"));
        assert!(prompt.contains("Do not treat update requests"));
    }
}
```

Append this module export to `src/lib.rs`:

```rust
pub mod comment_classify;
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test comment_classify
```

Expected: FAIL with unresolved items such as `comment_output_schema`, `CommentModelOutput`, `CommentSentiment`, `CommentIntent`, and `system_prompt`.

- [ ] **Step 3: Implement the minimal types and helpers**

Replace `src/comment_classify.rs` with:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSentiment {
    Positive,
    Negative,
    Neutral,
    Mixed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentIntent {
    StoryQuality,
    TranslationQuality,
    UpdateRequest,
    Spam,
    Social,
    Question,
}

#[derive(Debug, Deserialize)]
pub struct CommentClassifyRequest {
    pub comment_id: Option<String>,
    pub story_id: Option<String>,
    pub chapter_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct CommentClassifyResponse {
    pub comment_id: Option<String>,
    pub story_id: Option<String>,
    pub chapter_id: Option<String>,
    pub sentiment: CommentSentiment,
    pub intent: CommentIntent,
    pub strength: f32,
    pub confidence: f32,
    pub is_quality_signal: bool,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct CommentModelOutput {
    sentiment: CommentSentiment,
    intent: CommentIntent,
    strength: f32,
    confidence: f32,
    is_quality_signal: bool,
    reason: String,
}

fn comment_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "sentiment": {
                "type": "string",
                "enum": ["positive", "negative", "neutral", "mixed"]
            },
            "intent": {
                "type": "string",
                "enum": [
                    "story_quality",
                    "translation_quality",
                    "update_request",
                    "spam",
                    "social",
                    "question"
                ]
            },
            "strength": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0
            },
            "confidence": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0
            },
            "is_quality_signal": {
                "type": "boolean"
            },
            "reason": {
                "type": "string",
                "maxLength": 160
            }
        },
        "required": [
            "sentiment",
            "intent",
            "strength",
            "confidence",
            "is_quality_signal",
            "reason"
        ],
        "additionalProperties": false
    })
}

fn system_prompt() -> &'static str {
    "You classify Vietnamese and English story-reader comments for later story quality scoring. \
     Return only the required JSON object. Classify sentiment as positive, negative, neutral, or mixed. \
     Classify intent as story_quality, translation_quality, update_request, spam, social, or question. \
     Use story_quality only when the comment judges plot, pacing, characters, writing, worldbuilding, \
     ending, logic, or overall story enjoyment. Use translation_quality when the comment judges translation, \
     editing, names, grammar, or readability caused by the translation. Do not treat update requests, \
     short social reactions, thanks, chapter begging, or spam as story quality signals. \
     Set strength from 0.0 to 1.0 for how strong the sentiment is. Set confidence from 0.0 to 1.0. \
     Set is_quality_signal true only for story_quality comments with enough substance to affect ranking."
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_schema_contains_fixed_comment_labels() {
        let schema = comment_output_schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(
            schema["properties"]["sentiment"]["enum"],
            json!(["positive", "negative", "neutral", "mixed"])
        );
        assert_eq!(
            schema["properties"]["intent"]["enum"],
            json!([
                "story_quality",
                "translation_quality",
                "update_request",
                "spam",
                "social",
                "question"
            ])
        );
        assert_eq!(
            schema["required"],
            json!([
                "sentiment",
                "intent",
                "strength",
                "confidence",
                "is_quality_signal",
                "reason"
            ])
        );
    }

    #[test]
    fn model_output_deserializes_valid_payload() {
        let parsed: CommentModelOutput = serde_json::from_value(json!({
            "sentiment": "positive",
            "intent": "story_quality",
            "strength": 0.8,
            "confidence": 0.9,
            "is_quality_signal": true,
            "reason": "Praises the story quality."
        }))
        .unwrap();

        assert_eq!(parsed.sentiment, CommentSentiment::Positive);
        assert_eq!(parsed.intent, CommentIntent::StoryQuality);
        assert_eq!(parsed.strength, 0.8);
        assert_eq!(parsed.confidence, 0.9);
        assert!(parsed.is_quality_signal);
    }

    #[test]
    fn prompt_mentions_vietnamese_story_comment_policy() {
        let prompt = system_prompt();

        assert!(prompt.contains("Vietnamese"));
        assert!(prompt.contains("story_quality"));
        assert!(prompt.contains("translation_quality"));
        assert!(prompt.contains("update_request"));
        assert!(prompt.contains("Do not treat update requests"));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test comment_classify
```

Expected: PASS for the three `comment_classify` tests.

- [ ] **Step 5: Commit**

```bash
git add src/comment_classify.rs src/lib.rs
git commit -m "feat: add comment classification schema"
```

---

### Task 2: Add the LLM-backed Comment Classification Service

**Files:**
- Modify: `src/comment_classify.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `src/comment_classify.rs`:

```rust
    #[test]
    fn builds_response_with_request_metadata() {
        let req = CommentClassifyRequest {
            comment_id: Some("cmt-1".to_string()),
            story_id: Some("story-9".to_string()),
            chapter_id: Some("chapter-3".to_string()),
            text: "Truyen hay, main thong minh.".to_string(),
        };
        let output = CommentModelOutput {
            sentiment: CommentSentiment::Positive,
            intent: CommentIntent::StoryQuality,
            strength: 0.8,
            confidence: 0.9,
            is_quality_signal: true,
            reason: "Praises the protagonist and story.".to_string(),
        };

        let response = build_response(req, output);

        assert_eq!(response.comment_id.as_deref(), Some("cmt-1"));
        assert_eq!(response.story_id.as_deref(), Some("story-9"));
        assert_eq!(response.chapter_id.as_deref(), Some("chapter-3"));
        assert_eq!(response.sentiment, CommentSentiment::Positive);
        assert_eq!(response.intent, CommentIntent::StoryQuality);
        assert_eq!(response.strength, 0.8);
        assert_eq!(response.confidence, 0.9);
        assert!(response.is_quality_signal);
    }

    #[test]
    fn user_prompt_includes_comment_text_and_no_think_suffix() {
        let prompt = user_prompt("Hong chuong moi qua.");

        assert!(prompt.contains("Comment: Hong chuong moi qua."));
        assert!(prompt.ends_with("/no_think"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test comment_classify
```

Expected: FAIL with unresolved functions `build_response` and `user_prompt`.

- [ ] **Step 3: Implement the service function and helpers**

Update the top of `src/comment_classify.rs` imports:

```rust
use crate::llm::{ChatMessage, JsonSchemaFormat};
use crate::state::AppState;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
```

Add these functions above the test module:

```rust
pub async fn classify_comment(
    state: &AppState,
    req: CommentClassifyRequest,
) -> Result<CommentClassifyResponse> {
    let content = state
        .llm
        .chat_completion(
            vec![
                ChatMessage::system(system_prompt()),
                ChatMessage::user(user_prompt(&req.text)),
            ],
            Some(JsonSchemaFormat {
                name: "comment_classification",
                schema: comment_output_schema(),
            }),
        )
        .await?;

    let parsed: CommentModelOutput =
        serde_json::from_str(&content).context("model output did not match comment schema")?;

    Ok(build_response(req, parsed))
}

fn user_prompt(text: &str) -> String {
    format!("Comment: {text}\n/no_think")
}

fn build_response(
    req: CommentClassifyRequest,
    output: CommentModelOutput,
) -> CommentClassifyResponse {
    CommentClassifyResponse {
        comment_id: req.comment_id,
        story_id: req.story_id,
        chapter_id: req.chapter_id,
        sentiment: output.sentiment,
        intent: output.intent,
        strength: output.strength,
        confidence: output.confidence,
        is_quality_signal: output.is_quality_signal,
        reason: output.reason,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test comment_classify
```

Expected: PASS for all `comment_classify` tests.

- [ ] **Step 5: Commit**

```bash
git add src/comment_classify.rs
git commit -m "feat: classify story comments with llm"
```

---

### Task 3: Expose `POST /v1/comments/classify`

**Files:**
- Modify: `src/api/handlers.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Write the failing compile target**

Modify `src/api/handlers.rs` imports:

```rust
use crate::comment_classify::{self, CommentClassifyRequest, CommentClassifyResponse};
```

Add this handler after the existing generic `classify` handler:

```rust
pub async fn classify_comment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommentClassifyRequest>,
) -> Result<Json<CommentClassifyResponse>, ApiError> {
    let response = comment_classify::classify_comment(&state, req).await?;
    Ok(Json(response))
}
```

Modify `src/api/mod.rs` router:

```rust
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
```

- [ ] **Step 2: Run compile check**

Run:

```bash
cargo check
```

Expected: PASS. If this fails because `CommentModelOutput` tests cannot access private helpers, keep helpers private but inside the same module; Rust unit tests in the same file can access private items through `use super::*`.

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test comment_classify
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/api/handlers.rs src/api/mod.rs
git commit -m "feat: expose comment classification endpoint"
```

---

### Task 4: Document and Add Manual API Example

**Files:**
- Modify: `README.md`
- Modify: `restapi/api.http`

- [ ] **Step 1: Update README API list**

Add this bullet after the existing `/v1/classify` bullet:

```markdown
- `POST /v1/comments/classify` — classifies one story comment into sentiment, intent,
  strength, confidence, and whether it is a story-quality signal.
```

- [ ] **Step 2: Add HTTP example**

Append this section after the existing `### Classify text` example in `restapi/api.http`:

```http
### Classify story comment
POST {{host}}/v1/comments/classify HTTP/1.1
Content-Type: application/json

{
  "comment_id": "cmt-1",
  "story_id": "story-9",
  "chapter_id": "chapter-3",
  "text": "Truyen hay, main thong minh, cang doc cang cuon."
}
```

- [ ] **Step 3: Run formatting and tests**

Run:

```bash
cargo fmt --check
cargo test comment_classify
```

Expected: `cargo fmt --check` PASS and `cargo test comment_classify` PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md restapi/api.http
git commit -m "docs: document comment classification endpoint"
```

---

### Task 5: Manual Verification Against a Running Local Stack

**Files:**
- No source edits expected.

- [ ] **Step 1: Start the stack**

Run:

```bash
./scripts/dev-up.sh
```

Expected: API listens on `http://127.0.0.1:3000`.

- [ ] **Step 2: Confirm readiness**

Run:

```bash
curl -s http://127.0.0.1:3000/readyz
```

Expected:

```json
{"status":"ok","llama_server":true,"qdrant":true}
```

- [ ] **Step 3: Classify a positive story-quality comment**

Run:

```bash
curl -s http://127.0.0.1:3000/v1/comments/classify \
  -H 'Content-Type: application/json' \
  -d '{
    "comment_id": "cmt-positive",
    "story_id": "story-1",
    "chapter_id": "chapter-10",
    "text": "Truyen hay, main thong minh, tinh tiet logic va cang ve sau cang cuon."
  }'
```

Expected shape:

```json
{
  "comment_id": "cmt-positive",
  "story_id": "story-1",
  "chapter_id": "chapter-10",
  "sentiment": "positive",
  "intent": "story_quality",
  "strength": 0.7,
  "confidence": 0.7,
  "is_quality_signal": true,
  "reason": "..."
}
```

The exact numeric values may vary, but `sentiment`, `intent`, and `is_quality_signal` should match.

- [ ] **Step 4: Classify an update-request comment**

Run:

```bash
curl -s http://127.0.0.1:3000/v1/comments/classify \
  -H 'Content-Type: application/json' \
  -d '{
    "comment_id": "cmt-update",
    "story_id": "story-1",
    "chapter_id": "chapter-10",
    "text": "Hong chuong moi qua, ad ra nhanh di."
  }'
```

Expected shape:

```json
{
  "comment_id": "cmt-update",
  "story_id": "story-1",
  "chapter_id": "chapter-10",
  "sentiment": "neutral",
  "intent": "update_request",
  "strength": 0.3,
  "confidence": 0.7,
  "is_quality_signal": false,
  "reason": "..."
}
```

The exact numeric values may vary, but `intent` should be `update_request` and `is_quality_signal` should be `false`.

- [ ] **Step 5: Classify a translation complaint**

Run:

```bash
curl -s http://127.0.0.1:3000/v1/comments/classify \
  -H 'Content-Type: application/json' \
  -d '{
    "comment_id": "cmt-translation",
    "story_id": "story-1",
    "chapter_id": "chapter-10",
    "text": "Noi dung chac cung duoc nhung dich kho doc qua, ten nhan vat lung tung."
  }'
```

Expected shape:

```json
{
  "comment_id": "cmt-translation",
  "story_id": "story-1",
  "chapter_id": "chapter-10",
  "sentiment": "negative",
  "intent": "translation_quality",
  "strength": 0.6,
  "confidence": 0.7,
  "is_quality_signal": false,
  "reason": "..."
}
```

The exact numeric values may vary, but `intent` should be `translation_quality`. Keep `is_quality_signal` false because this should be scored separately from story quality.

- [ ] **Step 6: Run full verification**

Run:

```bash
cargo fmt --check
cargo test
cargo check
```

Expected: all commands PASS.

- [ ] **Step 7: Commit any verification fixes**

If verification required fixes, commit them:

```bash
git add src/comment_classify.rs src/api/handlers.rs src/api/mod.rs README.md restapi/api.http
git commit -m "fix: stabilize comment classification endpoint"
```

If no fixes were needed, skip this commit.

---

## Deferred Work

Do not include these in the first implementation unless real usage proves they are needed:

- `POST /v1/comments/classify-batch`
- persistent storage of classification results
- story-level aggregate scoring endpoint
- model-specific calibration dataset
- retry/backoff wrapper around malformed model output
- configurable model name, temperature, max tokens, and seed

These are likely useful, but the single-comment endpoint should land first so the classification contract can be tested with real comments.

## Self-Review

- Spec coverage: The plan implements the endpoint, fixed structured labels, Vietnamese comment policy, metadata passthrough, docs, and manual checks for story-quality, update-request, and translation-quality comments.
- Placeholder scan: No placeholder markers or unspecified "add tests" steps remain.
- Type consistency: Request/response type names are consistent across service, handler, route, docs, and tests.
