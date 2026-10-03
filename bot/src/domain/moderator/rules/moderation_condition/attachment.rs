//! Matching on what a message carries besides its text.
//!
//! A message has at most one attachment, so each of the four conditions asks
//! the same question about a different kind.

use crate::domain::moderator::ports::MessageAttachment;

fn describe(kind: MessageAttachment) -> &'static str {
    match kind {
        MessageAttachment::Image => "an image",
        MessageAttachment::Video => "a video",
        MessageAttachment::Voice => "a voice message",
        MessageAttachment::File => "a file",
    }
}

/// Evaluates whether the message carries an attachment of the `wanted` kind.
pub fn should_moderate(
    attachment: Option<MessageAttachment>,
    wanted: MessageAttachment,
) -> Option<String> {
    if attachment? == wanted {
        Some(format!("contains {}", describe(wanted)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
