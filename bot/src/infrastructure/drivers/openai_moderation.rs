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
use std::collections::HashMap;
use std::time::Duration;

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
    let _ = text;
    todo!("openai_moderation::request_body")
}

/// Reads OpenAI's answer: its HTTP status, its `Retry-After` header if any,
/// and its body.
pub fn parse_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> Result<RawModeration, ModerationApiError> {
    let _ = (status, retry_after, body);
    todo!("openai_moderation::parse_response")
}

/// [`ModerationApi`] over HTTPS.
pub struct HttpModerationApi {
    endpoint: String,
    timeout: Duration,
}

impl HttpModerationApi {
    pub fn new(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout,
        }
    }
}

#[async_trait]
impl ModerationApi for HttpModerationApi {
    async fn moderate(
        &self,
        api_key: &str,
        text: &str,
    ) -> Result<RawModeration, ModerationApiError> {
        let _ = (&self.endpoint, self.timeout, api_key, text);
        todo!("HttpModerationApi::moderate")
    }
}
