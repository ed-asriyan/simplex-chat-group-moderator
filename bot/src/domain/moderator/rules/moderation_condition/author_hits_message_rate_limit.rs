//! `AuthorHitsMessageRateLimit`: the author has been busier than the owner allows.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, Needs};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::rate_limit;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The author sent at least `message_count` messages in the last
/// `time_window_minutes`, this one included. 0 in either disables it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorHitsMessageRateLimit {
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub message_count: u32,
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub time_window_minutes: u32,
}

#[async_trait]
impl Condition for AuthorHitsMessageRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "author sent at least {} messages in {} min",
            self.message_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        filter::check(
            ctx.ports.activity_repo,
            &ctx.group_message.group.id,
            &ctx.group_message.author_id,
            self.message_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }

    fn needs(&self) -> Needs {
        Needs::author_messages(self.time_window_minutes)
    }
}
