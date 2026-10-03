//! `ExceedsMaxWords`: the message is longer than the owner allows.

use super::message_length;
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
        Ok(message_length::should_moderate_words(
            &ctx.group_message.text,
            self.max_words,
        ))
    }
}
