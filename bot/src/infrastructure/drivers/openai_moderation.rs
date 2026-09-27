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

type Err = Box<dyn Error + Send + Sync>;

#[cfg(test)]
mod tests;

pub const MODERATIONS_URL: &str = "https://api.openai.com/v1/moderations";
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

/// Why there is no [`RawModeration`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModerationApiError {
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
    /// Any other non-success status.
    Server { status: u16 },
    /// A success status with a body that is not a moderation result.
    Malformed(String),
    /// The request never got an HTTP answer: DNS, TLS, connection, timeout.
    Transport(String),
}

/// Asks OpenAI to moderate one text.
#[async_trait]
pub trait ModerationApi: Send + Sync {
    async fn moderate(
        &self,
        api_key: &str,
        text: &str,
    ) -> Result<RawModeration, ModerationApiError>;
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

/// Reads OpenAI's answer: its HTTP status, its `Retry-After` header if any,
/// and its body.
pub fn parse_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<RawModeration, ModerationApiError> {
    match status {
        200..=299 => {
            let response: ModerationResponse = serde_json::from_str(body)
                .map_err(|e| ModerationApiError::Malformed(e.to_string()))?;
            let result = response.results.into_iter().next().ok_or_else(|| {
                ModerationApiError::Malformed("the answer carries no result".to_string())
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
        401 => Err(ModerationApiError::Unauthorized),
        403 => Err(ModerationApiError::Forbidden),
        // The same status covers "no money" and "too fast"; only the body tells
        // them apart, and only one of them is worth waiting out.
        429 if is_insufficient_quota(body) => Err(ModerationApiError::InsufficientQuota),
        429 => Err(ModerationApiError::RateLimited {
            retry_after: retry_after.and_then(parse_retry_after),
        }),
        status => Err(ModerationApiError::Server { status }),
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

/// [`ModerationApi`] over HTTPS.
pub struct HttpModerationApi {
    client: reqwest::Client,
    endpoint: String,
}

impl HttpModerationApi {
    /// `timeout` bounds a whole request, connection included.
    pub fn new(endpoint: impl Into<String>, timeout: Duration) -> Result<Self, Err> {
        Ok(Self {
            client: reqwest::Client::builder().timeout(timeout).build()?,
            endpoint: endpoint.into(),
        })
    }
}

#[async_trait]
impl ModerationApi for HttpModerationApi {
    async fn moderate(
        &self,
        api_key: &str,
        text: &str,
    ) -> Result<RawModeration, ModerationApiError> {
        // reqwest's errors name the URL, never the headers, so the key stays
        // out of whatever gets logged.
        let transport = |e: reqwest::Error| ModerationApiError::Transport(e.to_string());
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request_body(text).to_string())
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
        parse_response(status, retry_after.as_deref(), &body)
    }
}
