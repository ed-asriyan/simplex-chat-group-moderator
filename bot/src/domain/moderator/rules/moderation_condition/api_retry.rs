//! The limits on [`ApiRetry`], how hard a condition that asks a remote AI
//! provider tries before it gives up. Shared by `FlaggedByOmniModeration`
//! (OpenAI) and `FlaggedByOpenRouterInstruction` (OpenRouter): the setting
//! means the same for both, and so do its limits. The type itself crosses the
//! `OpenAi` and `OpenRouter` ports, so it lives with the ports' types.

use crate::domain::moderator::ports::ApiRetry;

/// Most tries an owner may ask for: each one can wait out the full request
/// timeout, and the message waits with it. Mirrored by `max_attempts` in
/// `rules-schema.json`.
pub const MAX_API_ATTEMPTS: u32 = 5;

/// Longest pause an owner may ask for between two tries, in seconds. Mirrored
/// by `retry_delay_seconds` in `rules-schema.json`.
pub const MAX_API_RETRY_DELAY_SECONDS: u32 = 10;

/// Checked when an owner saves the rule.
pub fn validate(retry: &ApiRetry, title: &str) -> Result<(), String> {
    if !(1..=MAX_API_ATTEMPTS).contains(&retry.max_attempts) {
        return Err(format!(
            "'{title}' needs between 1 and {MAX_API_ATTEMPTS} attempts, got {}",
            retry.max_attempts
        ));
    }
    if retry.retry_delay_seconds > MAX_API_RETRY_DELAY_SECONDS {
        return Err(format!(
            "'{title}' can wait at most {MAX_API_RETRY_DELAY_SECONDS} seconds between attempts, got {}",
            retry.retry_delay_seconds
        ));
    }
    Ok(())
}
