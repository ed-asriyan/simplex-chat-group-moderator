//! `ModerateMessage`: delete the message that matched.

#[cfg(test)]
mod tests;

use super::{Action, ActionContext, ActionReport, Effect};
use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Deletes the message that matched, and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerateMessage {}

#[async_trait]
impl Action for ModerateMessage {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn effect(&self) -> Effect {
        Effect {
            message_deleted: true,
            ..Effect::default()
        }
    }

    fn execution_rank(&self) -> u8 {
        1
    }

    async fn execute(&self, ctx: &ActionContext<'_>) -> Result<ActionReport, Err> {
        let message = ctx.group_message;
        ctx.ports
            .group_moderator
            .delete_message(&message.group.id, &message.message_id)
            .await?;
        Ok(ActionReport::default())
    }
}
