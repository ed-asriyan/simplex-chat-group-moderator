//! What every OpenAI endpoint the bot calls has in common: how it says no,
//! and how a request is sent and retried.
//!
//! A driver: it speaks OpenAI's own words (HTTP statuses, `insufficient_quota`,
//! `Retry-After`) and holds no domain type. The endpoint clients —
//! [`super::openai_moderation`] and [`super::openai_responses`] — read their own
//! answers and share everything else from here.

use std::error::Error;
use std::future::Future;
use std::time::Duration;

use serde::Deserialize;

#[cfg(test)]
mod tests;

type Err = Box<dyn Error + Send + Sync>;

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
pub fn error_for_status(status: u16, retry_after: Option<&str>, body: &str) -> OpenAiApiError {
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

const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

fn retry_delay(attempt: usize) -> Duration {
    let multiplier = 1u64 << attempt.saturating_sub(1).min(4);
    DEFAULT_RETRY_DELAY
        .checked_mul(multiplier as u32)
        .unwrap_or(MAX_RETRY_DELAY)
        .min(MAX_RETRY_DELAY)
}

/// An HTTP answer, read far enough to be handed to an endpoint's parser.
pub struct HttpAnswer {
    pub status: u16,
    pub retry_after: Option<String>,
    pub body: String,
}

/// POSTs JSON to OpenAI with a key, retrying transient failures.
pub struct OpenAiHttp {
    client: reqwest::Client,
    max_attempts: usize,
}

impl OpenAiHttp {
    /// `timeout` bounds each attempt, connection included.
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
    pub async fn post<T>(
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

/// Runs `attempt` up to `max_attempts` times while it fails transiently,
/// waiting `Retry-After` when OpenAI named one and backing off otherwise.
pub async fn with_retries<T, F, Fut>(
    max_attempts: usize,
    mut attempt: F,
) -> Result<T, OpenAiApiError>
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
