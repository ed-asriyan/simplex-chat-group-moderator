//! `ContainsVideo`: what the message carries besides its text.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
use crate::domain::moderator::ports::MessageAttachment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message carries a video.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsVideo {}

impl Condition for ContainsVideo {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "contains a video".into()
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for ContainsVideo {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        Ok(carries(ctx.subject.attachment))
    }
}

/// Whether the message carries a video. A message has at most one attachment.
fn carries(attachment: Option<MessageAttachment>) -> Option<String> {
    (attachment == Some(MessageAttachment::Video)).then(|| "contains a video".to_string())
}
