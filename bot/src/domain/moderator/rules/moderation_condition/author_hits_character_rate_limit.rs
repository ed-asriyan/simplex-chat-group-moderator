//! `AuthorHitsCharacterRateLimit`: the author has been busier than the owner allows.

use super::character_rate_limit;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::rate_limit;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The author wrote at least `character_count` characters in the last
/// `time_window_minutes`, this message's own characters included. 0 in
/// either disables it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorHitsCharacterRateLimit {
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub character_count: u32,
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub time_window_minutes: u32,
}

#[async_trait]
impl Condition for AuthorHitsCharacterRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "author sent at least {} characters in {} min",
            self.character_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        character_rate_limit::check(
            ctx.character_activity_repo,
            &ctx.group_message.group.id,
            &ctx.group_message.author_id,
            self.character_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }
}
