//! `GroupHitsLineRateLimit`: the whole group has been busier than the owner allows.

use super::line_rate_limit;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::{checks, rate_limit};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The group's messages took at least `line_count` lines on screen in the
/// last `time_window_minutes`, from all its members, this message's own
/// lines included, with every line longer than `chars_per_line` characters
/// counted as several (0: no wrapping). Counted like
/// `AuthorHitsLineRateLimit`, on a counter of its own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupHitsLineRateLimit {
    pub line_count: u32,
    pub time_window_minutes: u32,
    pub chars_per_line: u32,
}

#[async_trait]
impl Condition for GroupHitsLineRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // Unlike their author counterparts, which predate this check and read 0
        // as "disabled", these never had a disabled state to keep.
        checks::check_nonzero(
            self.line_count,
            "'Group Hits Line Rate Limit' needs at least 1 line",
        )?;
        rate_limit::check_window(self.time_window_minutes, "Group Hits Line Rate Limit")
    }

    fn describe(&self) -> String {
        format!(
            "group received at least {} lines in {} min",
            self.line_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        line_rate_limit::check_group(
            ctx.group_line_activity_repo,
            &ctx.group_message.group.id,
            self.line_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }
}
