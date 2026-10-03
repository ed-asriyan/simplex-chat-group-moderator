//! `KickAuthor`: remove the author from the group.

#[cfg(test)]
mod tests;

use super::{Action, ActionContext, ActionReport, Effect, Silence};
use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Removes the author from the group, optionally with everything they wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KickAuthor {
    /// Whether to delete every message the author has sent in the group, as
    /// opposed to leaving their history in place.
    #[serde(default)]
    pub delete_all_messages: bool,
}

#[async_trait]
impl Action for KickAuthor {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    /// An author who is out of the group writes nothing more to it. Deleting
    /// every message they sent deletes the one that matched too.
    fn effect(&self) -> Effect {
        Effect {
            message_deleted: self.delete_all_messages,
            history_deleted: self.delete_all_messages,
            author_silenced: Silence::Forever,
            author_removed: true,
        }
    }

    fn execution_rank(&self) -> u8 {
        2
    }

    async fn execute(&self, ctx: &ActionContext<'_>) -> Result<ActionReport, Err> {
        let message = ctx.group_message;
        ctx.ports
            .group_moderator
            .kick_member(
                &message.group.id,
                &message.author_id,
                self.delete_all_messages,
            )
            .await?;
        // Nobody to restore once they are out of the group.
        let cancelled = ctx
            .ports
            .restores
            .delete_for_member(&message.group.id, &message.author_id)
            .await;
        Ok(ActionReport {
            bookkeeping_error: cancelled.err(),
        })
    }
}
