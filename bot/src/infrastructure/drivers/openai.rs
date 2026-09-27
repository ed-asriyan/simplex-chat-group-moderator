//! The client for OpenAI: one API key, two things asked of it.
//!
//! - **Moderation** (`POST /v1/moderations`, `omni-moderation-latest`): scores
//!   a text in OpenAI's categories.
//! - **Judgement** (`POST /v1/responses`): a chosen model reads an instruction
//!   and a text and answers, held to it by a strict JSON schema, whether the
//!   text is what the instruction describes, and why.
//!
//! A driver, not an adapter: it speaks OpenAI's own words — category names
//! like `hate/threatening`, HTTP statuses, `insufficient_quota` — and holds no
//! domain type. Turning its answers into verdicts, and deciding how often to
//! ask, is the gateway adapter's business.
//!
//! Building a request and reading its answer are pure functions, so OpenAI's
//! wire format is tested without a network; [`OpenAiApi`] is the seam the
//! gateway is tested through.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::time::Duration;

#[cfg(test)]
mod tests;

type Err = Box<dyn Error + Send + Sync>;

const MODERATIONS_URL: &str = "https://api.openai.com/v1/moderations";
const RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

const MODERATION_MODEL: &str = "omni-moderation-latest";

// ---------------------------------------------------------------------------
// The API
// ---------------------------------------------------------------------------

/// The moderation model's answer for one text, as OpenAI spells it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawModeration {
    pub flagged: bool,
    /// Category name → OpenAI's own yes/no.
    pub categories: HashMap<String, bool>,
    /// Category name → score, 0.0..=1.0.
    pub category_scores: HashMap<String, f64>,
}

/// A model's answer to an instruction about one text: whether the text is
/// what the instruction describes, and why.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Judgement {
    pub matches: bool,
    pub reason: String,
}

/// Why OpenAI gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenAiApiError {
    /// 401: OpenAI does not know the key.
    Unauthorized,
    /// 403: the key may not call this endpoint.
    Forbidden,
    /// 429 with `insufficient_quota`: no credits or no billing. Retrying does
    /// not help.
    InsufficientQuota,
    /// 429 for anything else: too many requests. `retry_after` is OpenAI's
    /// `Retry-After` header, when it sent one that reads as seconds.
    RateLimited { retry_after: Option<Duration> },
    /// Any other non-success status: 400 and 404 for a model the key may not
    /// use or a request the model does not support, 5xx for OpenAI's trouble.
    Server { status: u16 },
    /// A success status with a body that is not the answer asked for.
    Malformed(String),
    /// The request never got an HTTP answer: DNS, TLS, connection, timeout.
    Transport(String),
}

impl OpenAiApiError {
    /// Whether asking again can give a different answer. A key OpenAI does not
    /// know, may not use, or has no money behind stays that way.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Transport(_) => true,
            Self::Server { status } => *status >= 500,
            Self::Unauthorized | Self::Forbidden | Self::InsufficientQuota | Self::Malformed(_) => {
                false
            }
        }
    }
}

/// What the bot asks OpenAI, with the owner's key.
#[async_trait]
pub trait OpenAiApi: Send + Sync {
    /// The moderation model's scores for `text`.
    async fn moderate(&self, api_key: &str, text: &str) -> Result<RawModeration, OpenAiApiError>;

    /// Whether `model`, given `instruction`, says `text` is what it describes.
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
    ) -> Result<Judgement, OpenAiApiError>;
}

// ---------------------------------------------------------------------------
// Errors every endpoint shares
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(rename = "type")]
    kind: Option<String>,
    code: Option<String>,
}

const INSUFFICIENT_QUOTA: &str = "insufficient_quota";

/// Reads a non-success answer: its HTTP status, its `Retry-After` header if
/// any, and its body.
fn error_for_status(status: u16, retry_after: Option<&str>, body: &str) -> OpenAiApiError {
    match status {
        401 => OpenAiApiError::Unauthorized,
        403 => OpenAiApiError::Forbidden,
        // The same status covers "no money" and "too fast"; only the body tells
        // them apart, and only one of them is worth waiting out.
        429 if is_insufficient_quota(body) => OpenAiApiError::InsufficientQuota,
        429 => OpenAiApiError::RateLimited {
            retry_after: retry_after.and_then(parse_retry_after),
        },
        status => OpenAiApiError::Server { status },
    }
}

fn is_insufficient_quota(body: &str) -> bool {
    serde_json::from_str::<ErrorResponse>(body).is_ok_and(|response| {
        response.error.code.as_deref() == Some(INSUFFICIENT_QUOTA)
            || response.error.kind.as_deref() == Some(INSUFFICIENT_QUOTA)
    })
}

/// `Retry-After` as a number of seconds. The HTTP-date form is ignored: no
/// hint is better than a wrong one, and the gateway has a default.
fn parse_retry_after(value: &str) -> Option<Duration> {
    let seconds: f64 = value.trim().parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

// ---------------------------------------------------------------------------
// Moderation: the wire format
// ---------------------------------------------------------------------------

/// `{"model": "omni-moderation-latest", "input": text}`.
fn moderation_request_body(text: &str) -> serde_json::Value {
    serde_json::json!({ "model": MODERATION_MODEL, "input": text })
}

#[derive(Deserialize)]
struct ModerationResponse {
    results: Vec<ModerationResult>,
}

/// Values are optional on the way in so that one `null` OpenAI sends for a
/// category costs that category, not the whole verdict.
#[derive(Deserialize)]
struct ModerationResult {
    flagged: bool,
    categories: HashMap<String, Option<bool>>,
    category_scores: HashMap<String, Option<f64>>,
}

/// Reads the moderation endpoint's answer: its HTTP status, its `Retry-After`
/// header if any, and its body.
fn parse_moderation_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<RawModeration, OpenAiApiError> {
    if !(200..=299).contains(&status) {
        return Err(error_for_status(status, retry_after, body));
    }
    let response: ModerationResponse =
        serde_json::from_str(body).map_err(|e| OpenAiApiError::Malformed(e.to_string()))?;
    let result = response
        .results
        .into_iter()
        .next()
        .ok_or_else(|| OpenAiApiError::Malformed("the answer carries no result".to_string()))?;
    Ok(RawModeration {
        flagged: result.flagged,
        categories: result
            .categories
            .into_iter()
            .filter_map(|(name, flagged)| Some((name, flagged?)))
            .collect(),
        category_scores: result
            .category_scores
            .into_iter()
            .filter_map(|(name, score)| Some((name, score?)))
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Judgement: the wire format
// ---------------------------------------------------------------------------

/// What the answer's fields mean, as the model is told.
const MATCHES_DESCRIPTION: &str =
    "true if the message is one the instructions ask to catch, false otherwise";
const REASON_DESCRIPTION: &str =
    "Why, in one short sentence of at most 20 words, naming what in the message decided it";

/// Room for the verdict and a sentence of reason. An answer cut off by this
/// limit is no verdict, so it is generous next to what the schema asks for.
const MAX_OUTPUT_TOKENS: u32 = 200;

/// The instruction goes in as the system message and the text as the user
/// message: the model reads the text as data to judge, and the schema leaves it
/// nothing to answer with but `true` or `false` and a sentence saying why.
/// `temperature` 0 so the same text gets the same verdict; `store` false so
/// OpenAI keeps no copy of the conversation for later retrieval.
fn judgement_request_body(model: &str, instruction: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "input": [
            { "role": "system", "content": [{ "type": "input_text", "text": instruction }] },
            { "role": "user", "content": [{ "type": "input_text", "text": text }] }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "verdict",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "matches": { "type": "boolean", "description": MATCHES_DESCRIPTION },
                        "reason": { "type": "string", "description": REASON_DESCRIPTION }
                    },
                    "required": ["matches", "reason"],
                    "additionalProperties": false
                }
            }
        },
        "temperature": 0,
        "max_output_tokens": MAX_OUTPUT_TOKENS,
        "store": false
    })
}

#[derive(Deserialize)]
struct Response {
    status: Option<String>,
    #[serde(default)]
    output: Vec<OutputItem>,
}

#[derive(Deserialize)]
struct OutputItem {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Vec<ContentPart>,
}

#[derive(Deserialize)]
struct ContentPart {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    refusal: Option<String>,
}

/// Reads the Responses endpoint's answer. Anything but a completed answer
/// carrying the verdict — a refusal, an answer cut short, text that is not the
/// schema — is `Malformed`: no verdict, rather than a guessed one.
fn parse_judgement_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<Judgement, OpenAiApiError> {
    if !(200..=299).contains(&status) {
        return Err(error_for_status(status, retry_after, body));
    }
    let malformed = |why: String| OpenAiApiError::Malformed(why);
    let response: Response = serde_json::from_str(body).map_err(|e| malformed(e.to_string()))?;
    if let Some(status) = response.status.as_deref()
        && status != "completed"
    {
        return Err(malformed(format!("the answer is {status}")));
    }
    // Reasoning and other items may come first; the verdict is in a message.
    let parts = response
        .output
        .iter()
        .filter(|item| item.kind == "message")
        .flat_map(|item| item.content.iter());
    for part in parts {
        if let Some(refusal) = part.refusal.as_deref().filter(|_| part.kind == "refusal") {
            return Err(malformed(format!("the model refused: {refusal}")));
        }
        if part.kind == "output_text"
            && let Some(text) = part.text.as_deref()
        {
            return serde_json::from_str::<Judgement>(text)
                .map_err(|e| malformed(format!("the answer is not the verdict: {e}")));
        }
    }
    Err(malformed("the answer carries no verdict".to_string()))
}

// ---------------------------------------------------------------------------
// Over HTTPS
// ---------------------------------------------------------------------------

/// An HTTP answer, read far enough to be handed to an endpoint's parser.
struct HttpAnswer {
    status: u16,
    retry_after: Option<String>,
    body: String,
}

/// [`OpenAiApi`] over HTTPS, retrying transient failures.
pub struct HttpOpenAiApi {
    client: reqwest::Client,
    max_attempts: usize,
}

impl HttpOpenAiApi {
    /// `timeout` bounds each attempt, connection included; transient failures
    /// are tried up to `max_attempts` times.
    pub fn new(timeout: Duration, max_attempts: usize) -> Result<Self, Err> {
        if max_attempts == 0 {
            return Err("max_attempts must be greater than zero".into());
        }
        Ok(Self {
            client: reqwest::Client::builder().timeout(timeout).build()?,
            max_attempts,
        })
    }

    /// Sends `body` and reads the answer with `read` — the endpoint's own
    /// parser — asking again while the failure is transient.
    async fn post<T>(
        &self,
        url: &str,
        api_key: &str,
        body: &serde_json::Value,
        read: impl Fn(HttpAnswer) -> Result<T, OpenAiApiError>,
    ) -> Result<T, OpenAiApiError> {
        with_retries(self.max_attempts, |_| async {
            read(self.post_once(url, api_key, body).await?)
        })
        .await
    }

    async fn post_once(
        &self,
        url: &str,
        api_key: &str,
        body: &serde_json::Value,
    ) -> Result<HttpAnswer, OpenAiApiError> {
        // reqwest's errors name the URL, never the headers, so the key stays
        // out of whatever gets logged.
        let transport = |e: reqwest::Error| OpenAiApiError::Transport(e.to_string());
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
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
        Ok(HttpAnswer {
            status,
            retry_after,
            body,
        })
    }
}

#[async_trait]
impl OpenAiApi for HttpOpenAiApi {
    async fn moderate(&self, api_key: &str, text: &str) -> Result<RawModeration, OpenAiApiError> {
        self.post(
            MODERATIONS_URL,
            api_key,
            &moderation_request_body(text),
            |answer| {
                parse_moderation_response(
                    answer.status,
                    answer.retry_after.as_deref(),
                    &answer.body,
                )
            },
        )
        .await
    }

    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
    ) -> Result<Judgement, OpenAiApiError> {
        self.post(
            RESPONSES_URL,
            api_key,
            &judgement_request_body(model, instruction, text),
            |answer| {
                parse_judgement_response(answer.status, answer.retry_after.as_deref(), &answer.body)
            },
        )
        .await
    }
}

const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

fn retry_delay(attempt: usize) -> Duration {
    let multiplier = 1u64 << attempt.saturating_sub(1).min(4);
    DEFAULT_RETRY_DELAY
        .checked_mul(multiplier as u32)
        .unwrap_or(MAX_RETRY_DELAY)
        .min(MAX_RETRY_DELAY)
}

/// Runs `attempt` up to `max_attempts` times while it fails transiently,
/// waiting `Retry-After` when OpenAI named one and backing off otherwise.
async fn with_retries<T, F, Fut>(max_attempts: usize, mut attempt: F) -> Result<T, OpenAiApiError>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T, OpenAiApiError>>,
{
    let mut number = 1;
    loop {
        match attempt(number).await {
            Err(error) if error.is_transient() && number < max_attempts => {
                let delay = match &error {
                    OpenAiApiError::RateLimited {
                        retry_after: Some(delay),
                    } => (*delay).min(MAX_RETRY_DELAY),
                    _ => retry_delay(number),
                };
                tokio::time::sleep(delay).await;
                number += 1;
            }
            result => return result,
        }
    }
}
