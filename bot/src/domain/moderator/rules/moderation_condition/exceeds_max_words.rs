//! `ExceedsMaxWords`: the message is longer than the owner allows.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message has more than `max_words` words.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExceedsMaxWords {
    pub max_words: u32,
}

#[async_trait]
impl Condition for ExceedsMaxWords {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // Unlike `AuthorHitsMessageRateLimit`, where 0 means "disabled", a zero
        // here would store a rule that can never match while the owner believes
        // it protects the group.
        checks::check_nonzero(
            self.max_words,
            "'Message Exceeds Max Words' needs a maximum of at least 1 word",
        )
    }

    fn describe(&self) -> String {
        format!("has more than {} words", self.max_words)
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(should_moderate_words(
            &ctx.group_message.text,
            self.max_words,
        ))
    }
}

/// Matches a message with more than `max_words` whitespace-separated words.
fn should_moderate_words(message: &str, max_words: u32) -> Option<String> {
    // Validation rejects 0 on save; a stored 0 still never matches, so a bad
    // value is inert rather than matching every message.
    if max_words == 0 {
        return None;
    }
    let count = message.split_whitespace().count();
    (count > max_words as usize).then(|| format!("{count} words"))
}
