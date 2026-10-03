//! What of the group's earlier messages a model is shown next to the one it
//! judges: each with its author's display name, as the members see it. Ids
//! never leave the bot. A message with neither text nor an attachment says
//! nothing and is left out.

use crate::domain::moderator::ports::{InstructionContextMessage, RecentGroupMessage};

#[cfg(test)]
mod tests;

pub(super) fn for_model(earlier: &[RecentGroupMessage]) -> Vec<InstructionContextMessage> {
    earlier
        .iter()
        .filter(|message| message.attachment.is_some() || !message.text.trim().is_empty())
        .map(|message| InstructionContextMessage {
            author_name: message.author_name.clone(),
            text: message.text.clone(),
            attachment: message.attachment,
        })
        .collect()
}
