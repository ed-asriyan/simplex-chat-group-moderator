//! `AuthorJoinedRecently`: the author is new to the group.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The author joined the group less than `time_window_minutes` ago. Never
/// matches members who were already in the group when the bot joined,
/// since their join time is unknown.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorJoinedRecently {
    pub time_window_minutes: u32,
}

impl Condition for AuthorJoinedRecently {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::check_nonzero(
            self.time_window_minutes,
            "'Author Joined Recently' needs a time window of at least 1 minute",
        )
    }

    fn describe(&self) -> String {
        format!(
            "author joined less than {} min ago",
            self.time_window_minutes
        )
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for AuthorJoinedRecently {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        Ok(filter::should_moderate(
            ctx.subject.author_joined_at,
            ctx.subject.timestamp,
            self.time_window_minutes,
        ))
    }
}
