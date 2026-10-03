//! `GroupHitsMessageRateLimit`: the whole group has been busier than the owner allows.

use super::message_rate_limit;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::{checks, rate_limit};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The group received at least `message_count` messages in the last
/// `time_window_minutes`, from all its members, this one included.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupHitsMessageRateLimit {
    pub message_count: u32,
    pub time_window_minutes: u32,
}

#[async_trait]
impl Condition for GroupHitsMessageRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // Unlike their author counterparts, which predate this check and read 0
        // as "disabled", these never had a disabled state to keep.
        checks::check_nonzero(
            self.message_count,
            "'Group Hits Message Rate Limit' needs at least 1 message",
        )?;
        rate_limit::check_window(self.time_window_minutes, "Group Hits Message Rate Limit")
    }

    fn describe(&self) -> String {
        format!(
            "group received at least {} messages in {} min",
            self.message_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        message_rate_limit::check_group(
            ctx.group_activity_repo,
            &ctx.group_message.group.id,
            self.message_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }
}
