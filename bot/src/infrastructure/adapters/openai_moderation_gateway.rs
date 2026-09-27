//! The one way the bot talks to OpenAI's Moderation API: an in-process queue
//! in front of the [`ModerationApi`] driver.
//!
//! Implements both OpenAI ports of the moderator context. Every request, a
//! message to classify or a key to verify, becomes a job on one bounded queue
//! that a single dispatcher drains, so how fast the bot talks to OpenAI is
//! decided in one place:
//! - a token bucket per key (`requests_per_minute_per_key`) keeps a flooded
//!   group from burning its owner's quota — a job with no token is refused,
//!   not delayed;
//! - at most `max_pending_per_key` jobs of one key wait at a time, so one key
//!   cannot fill the queue for everyone else;
//! - at most `max_in_flight` HTTP requests run at once, over all keys;
//! - a key OpenAI answered 429 for rests until its `Retry-After` has passed;
//! - a key OpenAI refused (401/403) is refused locally for `rejected_key_ttl`
//!   instead of being sent again with every message;
//! - key checks jump the queue: an owner is waiting on them.
//!
//! Nothing here blocks the caller on a full queue: a job that cannot be taken
//! is an immediate `Err`, and a job that waits past its deadline is dropped
//! without being sent. The ports stay request/response, so this queue can later
//! move out of the process without the domain noticing.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::moderator::ports::{
    Err, KeyCheck, OpenAiKeyVerifier, OpenAiModerationClassifier, OpenAiModerationResult,
};
use crate::infrastructure::drivers::openai_moderation::ModerationApi;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct OpenAiModerationGatewayConfig {
    /// Requests one key may make per minute; also the size of its burst.
    pub requests_per_minute_per_key: u32,
    /// Jobs of one key that may wait for a free slot at the same time.
    pub max_pending_per_key: usize,
    /// HTTP requests running at the same time, over all keys.
    pub max_in_flight: usize,
    /// Jobs the queue holds before refusing new ones.
    pub queue_capacity: usize,
    /// How long a message waits for its verdict, queueing included.
    pub classify_deadline: Duration,
    /// How long a key check waits, queueing and its one retry included.
    pub verify_deadline: Duration,
    /// How long a key OpenAI refused is refused locally.
    pub rejected_key_ttl: Duration,
    /// How long a rate-limited key rests when OpenAI sent no `Retry-After`.
    pub default_rate_limit_cooldown: Duration,
}

impl Default for OpenAiModerationGatewayConfig {
    fn default() -> Self {
        Self {
            // Below the free tier's reported 250 requests a minute.
            requests_per_minute_per_key: 200,
            max_pending_per_key: 20,
            max_in_flight: 8,
            queue_capacity: 1_000,
            classify_deadline: Duration::from_secs(2),
            verify_deadline: Duration::from_secs(10),
            rejected_key_ttl: Duration::from_secs(10 * 60),
            default_rate_limit_cooldown: Duration::from_secs(20),
        }
    }
}

pub struct OpenAiModerationGateway {
    api: Arc<dyn ModerationApi>,
    config: OpenAiModerationGatewayConfig,
}

impl OpenAiModerationGateway {
    /// Starts the dispatcher on the current Tokio runtime.
    pub fn new(api: Arc<dyn ModerationApi>, config: OpenAiModerationGatewayConfig) -> Self {
        let _ = (&api, &config);
        todo!("OpenAiModerationGateway::new")
    }
}

#[async_trait]
impl OpenAiModerationClassifier for OpenAiModerationGateway {
    async fn classify(&self, api_key: &str, text: &str) -> Result<OpenAiModerationResult, Err> {
        let _ = (&self.api, &self.config, api_key, text);
        todo!("OpenAiModerationGateway::classify")
    }
}

#[async_trait]
impl OpenAiKeyVerifier for OpenAiModerationGateway {
    /// A key check asks OpenAI even when the key is being refused locally —
    /// the owner may just have fixed its permissions — and a `Valid` answer
    /// lifts that refusal. It is not charged to the key's token bucket, and a
    /// transient failure is retried once before it reads as `Unreachable`.
    async fn verify(&self, api_key: &str) -> KeyCheck {
        let _ = (&self.api, &self.config, api_key);
        todo!("OpenAiModerationGateway::verify")
    }
}
