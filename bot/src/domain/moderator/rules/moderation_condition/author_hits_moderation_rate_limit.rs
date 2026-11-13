//! `AuthorHitsModerationRateLimit`: the author has had more messages moderated
//! than the owner allows. The one condition whose answer depends on what the
//! other rules do with this message — see the pre-pass in `rules`.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, Needs, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
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
    fn needs(&self) -> Needs {
        Needs::author_moderations(self.time_window_minutes)
    }

    fn depends_on_other_rules(&self) -> bool {
        true
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for AuthorHitsModerationRateLimit {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        // This message counts toward the limit when another rule moderates
        // it: otherwise one clean message could never trip the limit.
        if ctx.moderated_by_other_rules {
            if self.message_count == 0 || self.time_window_minutes == 0 {
                return Ok(None);
            }
            let since =
                rate_limit::window_start(ctx.subject.timestamp, self.time_window_minutes);
            let count = ctx
                .ports
                .moderation_activity_repo
                .count_moderated_messages_since(
                    &ctx.subject.group.id,
                    &ctx.subject.author_id,
                    since,
                    ctx.subject.timestamp,
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
            ctx.ports.moderation_activity_repo,
            &ctx.subject.group.id,
            &ctx.subject.author_id,
            self.message_count,
            self.time_window_minutes,
            ctx.subject.timestamp,
        )
        .await
    }
}
