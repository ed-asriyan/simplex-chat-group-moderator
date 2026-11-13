//! `MatchesExactMessage`: the whole message equals one of the owner's texts.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Maximum length (in characters) of a single exact message.
const MAX_MESSAGE_LENGTH: usize = 1000;

/// The whole message equals one of `messages`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MatchesExactMessage {
    pub messages: Vec<String>,
    pub case_sensitive: bool,
}

impl Condition for MatchesExactMessage {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::normalize_list(&mut self.messages);
        checks::check_list(&self.messages, MAX_MESSAGE_LENGTH, "messages", "Message")
    }

    fn describe(&self) -> String {
        "exactly matches one of the listed texts".into()
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for MatchesExactMessage {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        Ok(filter::should_moderate(
            ctx.subject.text.trim(),
            &self.messages,
            self.case_sensitive,
        ))
    }
}
