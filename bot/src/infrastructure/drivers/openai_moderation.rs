//! A client for OpenAI's Moderation API (`POST /v1/moderations`).
//!
//! A driver, not an adapter: it speaks OpenAI's own words — category names
//! like `hate/threatening`, HTTP statuses, `insufficient_quota` — and holds no
//! domain type. Turning its answers into the moderator's verdicts, and deciding
//! how often to ask, is the gateway adapter's business.
//!
//! The HTTP call and the reading of its answer are split: [`request_body`] and
//! [`parse_response`] are pure, so OpenAI's wire format is tested without a
//! network, and [`ModerationApi`] is the seam the gateway is tested through.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use super::openai::{OpenAiApiError, OpenAiHttp, error_for_status};

type Err = Box<dyn Error + Send + Sync>;

#[cfg(test)]
mod tests;

const URL: &str = "https://api.openai.com/v1/moderations";
pub const MODERATION_MODEL: &str = "omni-moderation-latest";

/// OpenAI's answer for one input, as OpenAI spells it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawModeration {
    pub flagged: bool,
    /// Category name → OpenAI's own yes/no.
    pub categories: HashMap<String, bool>,
    /// Category name → score, 0.0..=1.0.
    pub category_scores: HashMap<String, f64>,
}

/// Asks OpenAI to moderate one text.
#[async_trait]
pub trait ModerationApi: Send + Sync {
    async fn moderate(&self, api_key: &str, text: &str) -> Result<RawModeration, OpenAiApiError>;
}

/// The JSON body for one text: `{"model": "omni-moderation-latest", "input": text}`.
pub fn request_body(text: &str) -> serde_json::Value {
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

/// Reads OpenAI's answer: its HTTP status, its `Retry-After` header if any,
/// and its body.
pub fn parse_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<RawModeration, OpenAiApiError> {
    match status {
        200..=299 => {
            let response: ModerationResponse =
                serde_json::from_str(body).map_err(|e| OpenAiApiError::Malformed(e.to_string()))?;
            let result = response.results.into_iter().next().ok_or_else(|| {
                OpenAiApiError::Malformed("the answer carries no result".to_string())
            })?;
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
        status => Err(error_for_status(status, retry_after, body)),
    }
}

/// [`ModerationApi`] over HTTPS.
pub struct HttpModerationApi {
    http: OpenAiHttp,
}

impl HttpModerationApi {
    /// `timeout` bounds each attempt; transient failures are tried up to
    /// `max_attempts` times.
    pub fn new(timeout: Duration, max_attempts: usize) -> Result<Self, Err> {
        Ok(Self {
            http: OpenAiHttp::new(timeout, max_attempts)?,
        })
    }
}

#[async_trait]
impl ModerationApi for HttpModerationApi {
    async fn moderate(&self, api_key: &str, text: &str) -> Result<RawModeration, OpenAiApiError> {
        self.http
            .post(URL, api_key, &request_body(text), |answer| {
                parse_response(answer.status, answer.retry_after.as_deref(), &answer.body)
            })
            .await
    }
}
