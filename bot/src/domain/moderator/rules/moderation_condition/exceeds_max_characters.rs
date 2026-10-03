//! `ExceedsMaxCharacters`: the message is longer than the owner allows.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message is longer than `max_characters` characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExceedsMaxCharacters {
    pub max_characters: u32,
}

#[async_trait]
impl Condition for ExceedsMaxCharacters {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // Unlike `AuthorHitsMessageRateLimit`, where 0 means "disabled", a zero
        // here would store a rule that can never match while the owner believes
        // it protects the group.
        checks::check_nonzero(
            self.max_characters,
            "'Message Exceeds Max Characters' needs a maximum of at least 1 character",
        )
    }

    fn describe(&self) -> String {
        format!("has more than {} characters", self.max_characters)
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(should_moderate_characters(
            &ctx.group_message.text,
            self.max_characters,
        ))
    }
}

/// Matches a message with more than `max_characters` characters. Whitespace
/// and line breaks count.
fn should_moderate_characters(message: &str, max_characters: u32) -> Option<String> {
    // Validation rejects 0 on save; a stored 0 still never matches, so a bad
    // value is inert rather than matching every message.
    if max_characters == 0 {
        return None;
    }
    let count = message.chars().count();
    (count > max_characters as usize).then(|| format!("{count} characters"))
}
