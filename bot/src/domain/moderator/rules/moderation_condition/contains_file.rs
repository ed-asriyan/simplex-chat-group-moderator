//! `ContainsFile`: what the message carries besides its text.

use super::attachment;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::ports::MessageAttachment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message carries a file that is not a picture, video or voice
/// message — those have conditions of their own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsFile {}

#[async_trait]
impl Condition for ContainsFile {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "contains a file".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(attachment::should_moderate(
            ctx.group_message.attachment,
            MessageAttachment::File,
        ))
    }
}
