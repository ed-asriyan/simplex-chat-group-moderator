//! `GroupHitsCharacterRateLimit`: the whole group has been busier than the owner allows.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, Needs};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::{checks, rate_limit};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The group's members wrote at least `character_count` characters in the
/// last `time_window_minutes`, all together, this message's own characters
/// included.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupHitsCharacterRateLimit {
    pub character_count: u32,
    pub time_window_minutes: u32,
}

#[async_trait]
impl Condition for GroupHitsCharacterRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // Unlike their author counterparts, which predate this check and read 0
        // as "disabled", these never had a disabled state to keep.
        checks::check_nonzero(
            self.character_count,
            "'Group Hits Character Rate Limit' needs at least 1 character",
        )?;
        rate_limit::check_window(self.time_window_minutes, "Group Hits Character Rate Limit")
    }

    fn describe(&self) -> String {
        format!(
            "group received at least {} characters in {} min",
            self.character_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        filter::check(
            ctx.ports.group_character_activity_repo,
            &ctx.group_message.group.id,
            self.character_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }

    fn needs(&self) -> Needs {
        Needs::group_characters(self.time_window_minutes)
    }
}
