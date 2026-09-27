//! A client for OpenAI's Responses API (`POST /v1/responses`), used for one
//! thing: asking a model whether a message is what an instruction describes,
//! with the answer held by a strict JSON schema to a boolean and a short reason.
//!
//! A driver: it speaks OpenAI's words and holds no domain type. Which model to
//! ask, and what the answer means for a group, is decided further in.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use std::time::Duration;

use super::openai::{OpenAiApiError, OpenAiHttp, error_for_status};

type Err = Box<dyn Error + Send + Sync>;

#[cfg(test)]
mod tests;

pub const RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

/// What the answer's fields mean, as the model is told.
const MATCHES_DESCRIPTION: &str =
    "true if the message is one the instructions ask to catch, false otherwise";
const REASON_DESCRIPTION: &str =
    "Why, in one short sentence of at most 20 words, naming what in the message decided it";

/// Room for the verdict and a sentence of reason. An answer cut off by this
/// limit is no verdict, so it is generous next to what the schema asks for.
const MAX_OUTPUT_TOKENS: u32 = 200;

/// A model's answer: whether the message is what the instruction describes,
/// and why.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Judgement {
    pub matches: bool,
    pub reason: String,
}

/// Asks a model whether a message is what an instruction describes.
#[async_trait]
pub trait ResponsesApi: Send + Sync {
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
    ) -> Result<Judgement, OpenAiApiError>;
}

/// The owner's instruction goes in as the system message and the group
/// message as the user message: the model reads the message as data to judge,
/// and the schema leaves it nothing to answer with but `true` or `false` and
/// a sentence saying why.
/// `temperature` 0 so the same message gets the same verdict; `store` false so
/// OpenAI keeps no copy of the conversation for later retrieval.
pub fn request_body(model: &str, instruction: &str, text: &str) -> serde_json::Value {
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

/// Reads OpenAI's answer: its HTTP status, its `Retry-After` header if any,
/// and its body. Anything but a completed answer carrying the verdict — a
/// refusal, an answer cut short, text that is not the schema — is `Malformed`:
/// no verdict, rather than a guessed one.
pub fn parse_response(
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

/// [`ResponsesApi`] over HTTPS.
pub struct HttpResponsesApi {
    http: OpenAiHttp,
    endpoint: String,
}

impl HttpResponsesApi {
    /// `timeout` bounds each attempt; transient failures are tried up to
    /// `max_attempts` times.
    pub fn new(
        endpoint: impl Into<String>,
        timeout: Duration,
        max_attempts: usize,
    ) -> Result<Self, Err> {
        Ok(Self {
            http: OpenAiHttp::new(timeout, max_attempts)?,
            endpoint: endpoint.into(),
        })
    }
}

#[async_trait]
impl ResponsesApi for HttpResponsesApi {
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
    ) -> Result<Judgement, OpenAiApiError> {
        self.http
            .post(
                &self.endpoint,
                api_key,
                &request_body(model, instruction, text),
                |answer| parse_response(answer.status, answer.retry_after.as_deref(), &answer.body),
            )
            .await
    }
}
