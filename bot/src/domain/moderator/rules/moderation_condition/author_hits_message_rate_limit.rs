//! `AuthorHitsMessageRateLimit`: the author has been busier than the owner allows.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, Needs, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
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
    fn needs(&self) -> Needs {
        Needs::author_messages(self.time_window_minutes)
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for AuthorHitsMessageRateLimit {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        filter::check(
            ctx.ports.activity_repo,
            &ctx.subject.group.id,
            &ctx.subject.author_id,
            self.message_count,
            self.time_window_minutes,
            ctx.subject.timestamp,
        )
        .await
    }
}
