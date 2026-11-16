//! `SendMemberMessage`: send the message's author the owner's own text, in the
//! group's chat with admins (US-F5, US-F6).
//!
//! SimpleX groups keep direct messages between members off by default, so the
//! text goes where an admin can always reach a member: the group's chat with
//! admins, which only the member and the group's admins see. It is sent before
//! every other action, while the member can still get it (US-F4).

#[cfg(test)]
mod tests;

use super::{Action, ActionContext, ActionOn, ActionReport, Effect};
use crate::domain::moderator::ports::{Err, GroupMessage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The longest text an owner may write, in characters (US-F5, US-C2).
pub const MAX_MEMBER_MESSAGE_CHARS: usize = 1000;

/// Sends the author `text`, exactly as the owner wrote it, in SimpleX Markdown
/// (US-F5). An empty text, or one longer than [`MAX_MEMBER_MESSAGE_CHARS`], is
/// refused when the owner saves the rule (US-C2). A message to the member
/// covers another one, so the member gets the first matched rule's alone
/// (US-F4). When it cannot reach the member, the rule's other actions are
/// carried out all the same, and the owner's notification names it as sent
/// (US-F6, US-D2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMemberMessage {
    pub text: String,
}

impl Action for SendMemberMessage {
    /// The text is the owner's, so it is checked and never changed. A text of
    /// spaces alone is as good as none: the editor holds the rule back for it too.
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        if self.text.trim().is_empty() {
            return Err("A private message to the member is empty. Write it, or remove the action.".into());
        }
        if self.text.chars().count() > MAX_MEMBER_MESSAGE_CHARS {
            return Err(format!(
                "A private message to the member is longer than {MAX_MEMBER_MESSAGE_CHARS} characters."
            )
            .into());
        }
        Ok(())
    }

    /// Achieves a thing no other action does, and is covered by none.
    fn effect(&self) -> Effect {
        Effect {
            member_messaged: true,
            ..Effect::default()
        }
    }

    /// First, while the member can still get it.
    fn execution_rank(&self) -> u8 {
        0
    }

    fn title(&self) -> &'static str {
        "Send the member a private message"
    }
}

#[async_trait]
impl ActionOn<GroupMessage> for SendMemberMessage {
    async fn execute(&self, ctx: &ActionContext<'_, GroupMessage>) -> Result<ActionReport, Err> {
        let message = ctx.subject;
        // A message that does not reach the member is no reason to leave the
        // rule's other actions undone, and no failure of the bot's own records
        // either: the adapter says why it did not reach them.
        let _ = ctx
            .ports
            .group_moderator
            .send_member_message(&message.group.id, &message.author_id, &self.text)
            .await;
        Ok(ActionReport::default())
    }
}
