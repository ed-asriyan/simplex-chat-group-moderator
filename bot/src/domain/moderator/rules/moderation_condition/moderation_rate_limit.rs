use crate::domain::moderator::ports::{
    Err, MessengerGroupId, UserId, UserModerationActivityRepository,
};
use crate::domain::moderator::rules::common::rate_limit::{reached, window_start};
use chrono::{DateTime, Utc};

/// Evaluates if at least `message_count` of the author's messages were
/// moderated in the window.
pub fn should_moderate(count: u32, message_count: u32, time_window_minutes: u32) -> Option<String> {
    if reached(count, message_count, time_window_minutes) {
        Some(format!(
            "author had {count} messages moderated in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Helper that queries the `UserModerationActivityRepository` and evaluates the condition.
pub async fn check(
    moderation_activity_repo: &dyn UserModerationActivityRepository,
    group_id: &MessengerGroupId,
    user_id: &UserId,
    message_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if message_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let since = window_start(now, time_window_minutes);
    let count = moderation_activity_repo
        .count_moderated_messages_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, message_count, time_window_minutes))
}

#[cfg(test)]
mod tests;
