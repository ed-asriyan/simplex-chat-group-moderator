//! `MatchesExactMessage`: the whole message equals one of the owner's texts.

use super::exact_message;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
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

#[async_trait]
impl Condition for MatchesExactMessage {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::normalize_list(&mut self.messages);
        checks::check_list(&self.messages, MAX_MESSAGE_LENGTH, "messages", "Message")
    }

    fn describe(&self) -> String {
        "exactly matches one of the listed texts".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(exact_message::should_moderate(
            ctx.group_message.text.trim(),
            &self.messages,
            self.case_sensitive,
        ))
    }
}
