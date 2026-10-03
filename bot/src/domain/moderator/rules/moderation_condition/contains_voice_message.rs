//! `ContainsVoiceMessage`: what the message carries besides its text.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::ports::MessageAttachment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message carries a voice message.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsVoiceMessage {}

#[async_trait]
impl Condition for ContainsVoiceMessage {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "contains a voice message".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(carries(ctx.group_message.attachment))
    }
}

/// Whether the message carries a voice message. A message has at most one attachment.
fn carries(attachment: Option<MessageAttachment>) -> Option<String> {
    (attachment == Some(MessageAttachment::Voice)).then(|| "contains a voice message".to_string())
}
