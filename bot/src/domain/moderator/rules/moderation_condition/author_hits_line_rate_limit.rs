//! `AuthorHitsLineRateLimit`: the author has been busier than the owner allows.

use super::line_rate_limit;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::rate_limit;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The author's messages took at least `line_count` lines on screen in the
/// last `time_window_minutes`, this message's own lines included, with
/// every line longer than `chars_per_line` characters counted as several
/// (0: no wrapping). 0 in the count or the window disables it.
///
/// Each message is counted once, when it arrives, with the width configured
/// then: the counter keeps a total, not the messages, so changing the width
/// only affects messages that come after it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorHitsLineRateLimit {
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub line_count: u32,
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub time_window_minutes: u32,
    #[serde(default, deserialize_with = "rate_limit::zero_if_null")]
    pub chars_per_line: u32,
}

#[async_trait]
impl Condition for AuthorHitsLineRateLimit {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "author sent at least {} lines in {} min",
            self.line_count, self.time_window_minutes
        )
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        // The wrap width is not read here: it did its work when the message was
        // counted into the window.
        line_rate_limit::check(
            ctx.line_activity_repo,
            &ctx.group_message.group.id,
            &ctx.group_message.author_id,
            self.line_count,
            self.time_window_minutes,
            ctx.group_message.timestamp,
        )
        .await
    }
}
