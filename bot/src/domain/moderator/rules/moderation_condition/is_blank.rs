//! `IsBlank`: the message is empty, or made only of whitespace and invisible
//! characters.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use crate::domain::moderator::rules::common::invisible::is_invisible;
use serde::{Deserialize, Serialize};

/// The message consists only of whitespace, line breaks and invisible
/// characters, or has no characters at all — and carries nothing else. A
/// message with an attachment is never blank, however empty its caption
/// is: a picture is not an empty message, it is a picture.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IsBlank {}

#[async_trait]
impl Condition for IsBlank {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "is empty or blank and carries no attachment".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        // A message that carries something is not an empty message, whatever
        // its caption says, so the attachment is checked before the text.
        if ctx.group_message.attachment.is_some() {
            return Ok(None);
        }
        Ok(should_moderate_blank(
            &ctx.group_message.text,
        ))
    }
}

fn is_blank(c: char) -> bool {
    c.is_whitespace() || is_invisible(c)
}

/// Matches a message that consists only of whitespace, line breaks and
/// invisible characters — including one with no characters at all.
fn should_moderate_blank(message: &str) -> Option<String> {
    message
        .chars()
        .all(is_blank)
        .then(|| "empty message".to_string())
}
