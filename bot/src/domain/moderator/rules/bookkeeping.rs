//! What the bot records about every message so the conditions can answer
//! later: the counters the rate limits read, the author's moderated messages,
//! and the group's latest messages.
//!
//! Nothing here knows a condition by name. Each one says what it reads
//! ([`Needs`]), the needs of every condition of every rule are merged, and a
//! message is recorded once into each counter that something reads, for as
//! long as the longest window asks.

use std::time::Duration;

use super::ModerationRule;
use super::common::rate_limit::MAX_RATE_LIMIT_WINDOW_MINUTES;
use super::common::screen_lines::count_effective_lines;
use super::moderation_condition::{ConditionPorts, Needs};
use crate::domain::moderator::ports::{Err, GroupMessage, RecentGroupMessage};

#[cfg(test)]
mod tests;

/// What every condition of every rule needs recorded, together.
fn needs_of(rules: &[ModerationRule]) -> Needs {
    rules.iter().fold(Needs::default(), |needs, rule| {
        needs.merge(rule.condition.needs())
    })
}

/// How long a counter keeps what it is given. The counters keep nothing past
/// the longest window a rate limit may count over.
fn ttl(time_window_minutes: u32) -> Duration {
    Duration::from_secs(u64::from(time_window_minutes.min(MAX_RATE_LIMIT_WINDOW_MINUTES)) * 60)
}

/// Saturating: a message longer than `u32::MAX` characters cannot reach the
/// bot, and a wrapped count would read as a short message.
fn characters(text: &str) -> u32 {
    text.chars().count().min(u32::MAX as usize) as u32
}

/// An attachment adds no lines of its own — a caption-less picture weighs
/// nothing, where counting it as the one empty line it technically is would
/// make the limit count attachments.
fn lines(text: &str, chars_per_line: u32) -> u32 {
    if text.is_empty() {
        0
    } else {
        count_effective_lines(text, chars_per_line).min(u32::MAX as usize) as u32
    }
}

/// Counts a new message into every counter some rule reads, before the rules
/// run, so a rate limit counts the message it is judging. An edit is not new
/// traffic and feeds no counter.
pub async fn record_activity(
    message: &GroupMessage,
    rules: &[ModerationRule],
    ports: &ConditionPorts<'_>,
) -> Result<(), Err> {
    if message.is_edit {
        return Ok(());
    }
    let needs = needs_of(rules);
    let group_id = &message.group.id;
    let author_id = &message.author_id;
    let at = message.timestamp;

    if let Some(window) = needs.author_messages {
        ports
            .activity_repo
            .record_message(group_id, author_id, at, ttl(window))
            .await?;
    }
    if let Some(window) = needs.author_characters {
        ports
            .character_activity_repo
            .record_characters(
                group_id,
                author_id,
                at,
                characters(&message.text),
                ttl(window),
            )
            .await?;
    }
    if let Some(window) = needs.author_lines {
        ports
            .line_activity_repo
            .record_lines(
                group_id,
                author_id,
                at,
                lines(&message.text, window.chars_per_line),
                ttl(window.time_window_minutes),
            )
            .await?;
    }
    if let Some(window) = needs.group_messages {
        ports
            .group_activity_repo
            .record_message(group_id, at, ttl(window))
            .await?;
    }
    if let Some(window) = needs.group_characters {
        ports
            .group_character_activity_repo
            .record_characters(group_id, at, characters(&message.text), ttl(window))
            .await?;
    }
    if let Some(window) = needs.group_lines {
        ports
            .group_line_activity_repo
            .record_lines(
                group_id,
                at,
                lines(&message.text, window.chars_per_line),
                ttl(window.time_window_minutes),
            )
            .await?;
    }
    Ok(())
}

/// Counts a new message the rules moderated toward the author's moderated
/// messages, when some rule reads them. An edit was counted as the message it
/// edits.
pub async fn record_moderated(
    message: &GroupMessage,
    rules: &[ModerationRule],
    ports: &ConditionPorts<'_>,
) -> Result<(), Err> {
    if message.is_edit {
        return Ok(());
    }
    if let Some(window) = needs_of(rules).author_moderations {
        ports
            .moderation_activity_repo
            .record_moderated_message(
                &message.group.id,
                &message.author_id,
                message.timestamp,
                ttl(window),
            )
            .await?;
    }
    Ok(())
}

/// Keeps the message among the group's latest while some rule reads earlier
/// messages, as many as the most demanding one asks for. Called once the
/// message has been moderated: it is no context for itself, and one the bot
/// `deleted` is gone from the chat the members see.
pub async fn record_outcome(
    message: &GroupMessage,
    rules: &[ModerationRule],
    deleted: bool,
    ports: &ConditionPorts<'_>,
) -> Result<(), Err> {
    let Some(keep) = needs_of(rules).history else {
        return Ok(());
    };
    let group_id = &message.group.id;
    if deleted {
        // A new message was never recorded; an edit may have been.
        if message.is_edit {
            ports
                .message_history
                .forget_message(group_id, &message.message_id)
                .await?;
        }
        return Ok(());
    }
    let recent = RecentGroupMessage {
        message_id: message.message_id,
        author_id: message.author_id,
        author_name: message.author_name.clone(),
        text: message.text.clone(),
        attachment: message.attachment,
        timestamp: message.timestamp,
    };
    if message.is_edit {
        ports.message_history.record_edit(group_id, recent).await
    } else {
        ports
            .message_history
            .record_message(group_id, recent, keep)
            .await
    }
}
