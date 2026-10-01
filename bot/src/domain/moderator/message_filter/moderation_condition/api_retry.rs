//! How hard a condition that asks a remote AI provider tries before it gives
//! up. Shared by `FlaggedByOmniModeration` (OpenAI) and
//! `FlaggedByOpenRouterInstruction` (OpenRouter): the setting means the same
//! for both, and so do its limits.

use serde::{Deserialize, Serialize};

/// Most tries an owner may ask for: each one can wait out the full request
/// timeout, and the message waits with it. Mirrored by `max_attempts` in
/// `rules-schema.json`.
pub const MAX_API_ATTEMPTS: u32 = 5;

/// Longest pause an owner may ask for between two tries, in seconds. Mirrored
/// by `retry_delay_seconds` in `rules-schema.json`.
pub const MAX_API_RETRY_DELAY_SECONDS: u32 = 10;

/// How hard a condition tries its provider — OpenAI or OpenRouter — before
/// reading a failure as "no verdict": `max_attempts` calls in all (1 means no
/// retry), `retry_delay_seconds` apart. Only a failure that can pass — the
/// provider's trouble, a timeout, rate limiting — is tried again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiRetry {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_retry_delay_seconds")]
    pub retry_delay_seconds: u32,
}

fn default_max_attempts() -> u32 {
    3
}

fn default_retry_delay_seconds() -> u32 {
    1
}

impl Default for ApiRetry {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            retry_delay_seconds: default_retry_delay_seconds(),
        }
    }
}

impl ApiRetry {
    /// One call, never repeated.
    pub const NONE: Self = Self {
        max_attempts: 1,
        retry_delay_seconds: 0,
    };

    /// Checked when an owner saves the rule.
    pub fn validate(&self, title: &str) -> Result<(), String> {
        if !(1..=MAX_API_ATTEMPTS).contains(&self.max_attempts) {
            return Err(format!(
                "'{title}' needs between 1 and {MAX_API_ATTEMPTS} attempts, got {}",
                self.max_attempts
            ));
        }
        if self.retry_delay_seconds > MAX_API_RETRY_DELAY_SECONDS {
            return Err(format!(
                "'{title}' can wait at most {MAX_API_RETRY_DELAY_SECONDS} seconds between attempts, got {}",
                self.retry_delay_seconds
            ));
        }
        Ok(())
    }
}
