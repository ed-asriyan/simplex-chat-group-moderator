//! `ContainsFile`: what the message carries besides its text.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
use crate::domain::moderator::ports::MessageAttachment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message carries a file that is not a picture, video or voice
/// message — those have conditions of their own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsFile {}

impl Condition for ContainsFile {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "contains a file".into()
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for ContainsFile {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        Ok(carries(ctx.subject.attachment))
    }
}

/// Whether the message carries a file. A message has at most one attachment.
fn carries(attachment: Option<MessageAttachment>) -> Option<String> {
    (attachment == Some(MessageAttachment::File)).then(|| "contains a file".to_string())
}
