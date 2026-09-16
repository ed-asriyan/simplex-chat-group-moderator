//! Matching on what a message carries besides its text.
//!
//! A message has at most one attachment, so each of the four conditions asks
//! the same question about a different kind. The pairing lives here rather than
//! in the condition enum so that a kind the messenger gains later has one place
//! to be taught about.

use super::ModerationCondition;
use crate::domain::moderator::ports::MessageAttachment;

/// The attachment kind `condition` looks for, or `None` if it is not an
/// attachment condition at all.
fn wanted_kind(condition: &ModerationCondition) -> Option<MessageAttachment> {
    match condition {
        ModerationCondition::ContainsImage => Some(MessageAttachment::Image),
        ModerationCondition::ContainsVideo => Some(MessageAttachment::Video),
        ModerationCondition::ContainsVoiceMessage => Some(MessageAttachment::Voice),
        ModerationCondition::ContainsFile => Some(MessageAttachment::File),
        _ => None,
    }
}

fn describe(kind: MessageAttachment) -> &'static str {
    match kind {
        MessageAttachment::Image => "an image",
        MessageAttachment::Video => "a video",
        MessageAttachment::Voice => "a voice message",
        MessageAttachment::File => "a file",
    }
}

/// Evaluates whether the message carries the attachment kind `condition` asks
/// about.
pub fn should_moderate(
    attachment: Option<MessageAttachment>,
    condition: &ModerationCondition,
) -> Option<String> {
    let wanted = wanted_kind(condition)?;
    if attachment? == wanted {
        Some(format!("contains {}", describe(wanted)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
