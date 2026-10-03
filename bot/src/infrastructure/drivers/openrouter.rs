//! The client for OpenRouter: a model, chosen by the owner, reads an
//! instruction and a text and answers, held to it by a strict JSON schema,
//! whether the text is what the instruction describes, and why
//! (`POST /api/v1/chat/completions`, with the owner's API key).
//!
//! A driver, not an adapter: it speaks OpenRouter's own words — model slugs
//! like `openai/gpt-4o-mini`, HTTP statuses, the moderation metadata on a 403 —
//! and holds no domain type. Turning its answers into verdicts, and deciding
//! how often to ask, is the gateway adapter's business.
//!
//! Building a request and reading its answer are pure functions, so
//! OpenRouter's wire format is tested without a network; [`OpenRouterApi`] is
//! the seam the gateway is tested through.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use std::time::Duration;

#[cfg(test)]
mod tests;

type Err = Box<dyn Error + Send + Sync>;

const CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// How the bot appears in the owner's activity log on OpenRouter.
const APP_TITLE: &str = "SimpleX Moderator Bot";

// ---------------------------------------------------------------------------
// The API
// ---------------------------------------------------------------------------

/// A model's answer to an instruction about one text: whether the text is
/// what the instruction describes, and why.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Judgement {
    /// Whether the message should be deleted: the text matches the instruction.
    pub delete: bool,
    pub reason: String,
}

/// A message the model reads for context but does not judge. `author` is
/// the display name its author chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorMessage {
    pub author: String,
    pub text: String,
}

/// Why OpenRouter gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenRouterApiError {
    /// 401: OpenRouter does not know the key, or it was disabled.
    Unauthorized,
    /// 402: the account has no credits left, or the key has spent its own
    /// credit limit. Retrying does not help.
    InsufficientCredits,
    /// 403 for the key: its guardrail does not allow the model.
    Forbidden,
    /// 403 for the text: the moderation OpenRouter runs in front of some
    /// models (OpenAI's, Anthropic's) flagged it, and the model never read it.
    /// Says nothing about the key; asking again gives the same answer.
    InputFlagged { reasons: Vec<String> },
    /// 429: too many requests. `retry_after` is OpenRouter's `Retry-After`
    /// header, when it sent one that reads as seconds.
    RateLimited { retry_after: Option<Duration> },
    /// Any other non-success status: 400 for a request the model does not
    /// take, 404 for a model with no endpoint that takes it, 408 for a request
    /// that took too long, 5xx for OpenRouter's or the provider's trouble.
    Server { status: u16 },
    /// A success status with a body that is not the answer asked for.
    Malformed(String),
    /// The request never got an HTTP answer: DNS, TLS, connection, timeout.
    Transport(String),
}

impl OpenRouterApiError {
    /// Whether asking again can give a different answer. A key OpenRouter does
    /// not know, may not use the model, or has no credits behind stays that
    /// way, and a flagged text stays flagged.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Transport(_) => true,
            Self::Server { status } => *status == 408 || *status >= 500,
            Self::Unauthorized
            | Self::InsufficientCredits
            | Self::Forbidden
            | Self::InputFlagged { .. }
            | Self::Malformed(_) => false,
        }
    }
}

/// What the bot asks OpenRouter, with the owner's key.
#[async_trait]
pub trait OpenRouterApi: Send + Sync {
    /// Whether `model`, given `instruction`, says `text` is what it describes,
    /// having read `context` (oldest first) before it.
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        author_name: &str,
        text: &str,
        context: &[PriorMessage],
    ) -> Result<Judgement, OpenRouterApiError>;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// `{"error": {"code": 403, "message": "...", "metadata": {...}}}`, both as a
/// non-success answer and as a 200 whose provider failed mid-generation.
#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: Option<u16>,
    #[serde(default)]
    metadata: Option<ErrorMetadata>,
}

/// Only a moderation refusal carries `reasons`.
#[derive(Deserialize)]
struct ErrorMetadata {
    reasons: Option<Vec<String>>,
}

/// Reads a non-success answer: its HTTP status, its `Retry-After` header if
/// any, and its body.
fn error_for_status(status: u16, retry_after: Option<&str>, body: &str) -> OpenRouterApiError {
    match status {
        401 => OpenRouterApiError::Unauthorized,
        402 => OpenRouterApiError::InsufficientCredits,
        // The same status covers "this key may not" and "this text may not";
        // only the body tells them apart, and only one of them is the key's.
        403 => match flagged_reasons(body) {
            Some(reasons) => OpenRouterApiError::InputFlagged { reasons },
            None => OpenRouterApiError::Forbidden,
        },
        429 => OpenRouterApiError::RateLimited {
            retry_after: retry_after.and_then(parse_retry_after),
        },
        status => OpenRouterApiError::Server { status },
    }
}

fn flagged_reasons(body: &str) -> Option<Vec<String>> {
    serde_json::from_str::<ErrorResponse>(body)
        .ok()?
        .error
        .metadata?
        .reasons
}

/// `Retry-After` as a number of seconds. The HTTP-date form is ignored: no
/// hint is better than a wrong one, and the gateway has a default.
fn parse_retry_after(value: &str) -> Option<Duration> {
    let seconds: f64 = value.trim().parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

// ---------------------------------------------------------------------------
// Judgement: the wire format
// ---------------------------------------------------------------------------

/// What the answer's fields mean, as the model is told.
const DELETE_DESCRIPTION: &str = "Whether the message should be deleted.";
const REASON_DESCRIPTION: &str = "The reason why the message should or should not be deleted.";

/// Appended to the instruction when earlier messages come along, so the model
/// knows which text it judges.
fn context_note() -> String {
    "The user message is a JSON object. Judge only its `message`. \
     `earlier_messages` are the messages posted in the same group just before it, \
     oldest first, in the same shape: `author` is the display name of who wrote it, \
     `text` is what they wrote. Read them only to understand `message`; never judge them."
        .to_string()
}

/// The instruction goes in as the system message and the text as the user
/// message: the model reads the text as data to judge, and the schema leaves it
/// nothing to answer with but `true` or `false` and a sentence saying why.
/// `temperature` 0 so the same text gets the same verdict.
///
/// With `context`, the user message becomes one JSON object carrying the text
/// and the earlier messages, each in the same `{author, text}` shape, and the
/// instruction says so: being JSON, no
/// member can write a line that passes for another member's message, or for
/// the one judged.
///
/// `provider` is what OpenRouter routes by: `require_parameters` sends the
/// request only to a provider of the model that honours the schema and the
/// temperature, rather than to one that would silently ignore them, and
/// `data_collection: deny` only to one that does not keep the group's messages
/// to train on.
fn judgement_request_body(
    model: &str,
    instruction: &str,
    author_name: &str,
    text: &str,
    context: &[PriorMessage],
) -> serde_json::Value {
    let (system, user) = if context.is_empty() {
        (instruction.to_string(), text.to_string())
    } else {
        let message =
            |author: &str, text: &str| serde_json::json!({ "author": author, "text": text });
        let earlier: Vec<serde_json::Value> = context
            .iter()
            .map(|prior| message(&prior.author, &prior.text))
            .collect();
        (
            format!("{instruction}\n\n{}", context_note()),
            serde_json::json!({
                "earlier_messages": earlier,
                "message": message(author_name, text),
            })
            .to_string(),
        )
    };
    serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "message_delete_decision",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "delete": { "type": "boolean", "description": DELETE_DESCRIPTION },
                        "reason": { "type": "string", "description": REASON_DESCRIPTION }
                    },
                    "required": ["delete", "reason"],
                    "additionalProperties": false
                }
            }
        },
        "temperature": 0,
        "provider": {
            "require_parameters": true,
            "data_collection": "deny"
        }
    })
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    choices: Vec<Choice>,
    error: Option<ErrorBody>,
}

#[derive(Deserialize)]
struct Choice {
    finish_reason: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
    refusal: Option<String>,
}

/// Reads the chat completion. Anything but a finished answer carrying the
/// verdict — a refusal, an answer cut short, a provider that failed after
/// OpenRouter said 200, text that is not the schema — is no verdict, rather
/// than a guessed one.
fn parse_judgement_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<Judgement, OpenRouterApiError> {
    if !(200..=299).contains(&status) {
        return Err(error_for_status(status, retry_after, body));
    }
    let malformed = |why: String| OpenRouterApiError::Malformed(why);
    let response: Response = serde_json::from_str(body).map_err(|e| malformed(e.to_string()))?;
    // OpenRouter answers 200 as soon as a provider takes the request; if the
    // provider fails after that, the body is the error alone.
    if let Some(error) = response.error {
        return Err(match error.code {
            Some(code) if !(200..=299).contains(&code) => error_for_status(code, None, body),
            _ => malformed("the answer is an error".to_string()),
        });
    }
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| malformed("the answer carries no choice".to_string()))?;
    if let Some(reason) = choice.finish_reason.as_deref()
        && reason != "stop"
    {
        return Err(malformed(format!("the answer ended with {reason}")));
    }
    let message = choice
        .message
        .ok_or_else(|| malformed("the answer carries no message".to_string()))?;
    if let Some(refusal) = message.refusal.filter(|refusal| !refusal.is_empty()) {
        return Err(malformed(format!("the model refused: {refusal}")));
    }
    let text = message
        .content
        .ok_or_else(|| malformed("the answer carries no verdict".to_string()))?;
    serde_json::from_str::<Judgement>(text.trim())
        .map_err(|e| malformed(format!("the answer is not the verdict: {e}")))
}

// ---------------------------------------------------------------------------
// Over HTTPS
// ---------------------------------------------------------------------------

/// [`OpenRouterApi`] over HTTPS. One try per call: how often a message is
/// tried again is the owner's setting, and the gateway does the retrying so
/// that every try is paced against the key.
pub struct HttpOpenRouterApi {
    client: reqwest::Client,
}

impl HttpOpenRouterApi {
    /// `timeout` bounds each request, connection included.
    pub fn new(timeout: Duration) -> Result<Self, Err> {
        Ok(Self {
            client: reqwest::Client::builder().timeout(timeout).build()?,
        })
    }
}

#[async_trait]
impl OpenRouterApi for HttpOpenRouterApi {
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        author_name: &str,
        text: &str,
        context: &[PriorMessage],
    ) -> Result<Judgement, OpenRouterApiError> {
        // reqwest's errors name the URL, never the headers, so the key stays
        // out of whatever gets logged.
        let transport = |e: reqwest::Error| OpenRouterApiError::Transport(e.to_string());
        let response = self
            .client
            .post(CHAT_COMPLETIONS_URL)
            .bearer_auth(api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("X-Title", APP_TITLE)
            .body(
                judgement_request_body(model, instruction, author_name, text, context).to_string(),
            )
            .send()
            .await
            .map_err(transport)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response.text().await.map_err(transport)?;
        parse_judgement_response(status, retry_after.as_deref(), &body)
    }
}
