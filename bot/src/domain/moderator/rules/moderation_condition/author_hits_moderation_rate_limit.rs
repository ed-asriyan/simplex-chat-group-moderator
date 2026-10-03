//! `AuthorHitsModerationRateLimit`: the author has had more messages moderated
//! than the owner allows. The one condition whose answer depends on what the
//! other rules do with this message — see the pre-pass in `rules`.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::rate_limit;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// At least `message_count` of the author's messages were moderated in the
/// last `time_window_minutes`, this one included if another rule moderates
/// it. 0 in either disables it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorHitsModerationRateLimit {
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub message_count: u32,
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub time_window_minutes: u32,
}

#[async_trait]
impl Condition for AuthorHitsModerationRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "author had at least {} messages moderated in {} min",
            self.message_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        if ctx.moderation_rate_limit_pinned {
            return Ok(None);
        }
        if ctx.message_is_moderated {
            if self.message_count == 0 || self.time_window_minutes == 0 {
                return Ok(None);
            }
            let since =
                rate_limit::window_start(ctx.group_message.timestamp, self.time_window_minutes);
            let count = ctx
                .moderation_activity_repo
                .count_moderated_messages_since(
                    &ctx.group_message.group.id,
                    &ctx.group_message.author_id,
                    since,
                    ctx.group_message.timestamp,
                )
                .await?
                + 1;
            return Ok(filter::should_moderate(
                count,
                self.message_count,
                self.time_window_minutes,
            ));
        }
        filter::check(
            ctx.moderation_activity_repo,
            &ctx.group_message.group.id,
            &ctx.group_message.author_id,
            self.message_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }
}
