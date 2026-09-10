use crate::domain::moderator::ports::{
    Err, MessengerGroupId, UserId, UserModerationActivityRepository,
};
use chrono::{DateTime, Duration, Utc};

/// Evaluates if the user exceeds the moderation rate limit (number of moderated messages in time window).
pub fn should_moderate(count: u32, message_count: u32, time_window_minutes: u32) -> Option<String> {
    if message_count > 0 && time_window_minutes > 0 && count >= message_count {
        Some(format!(
            "user exceeds moderation rate limit: {count} moderated messages in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Helper that queries the `UserModerationActivityRepository` and evaluates the moderation rate limit.
pub async fn check_moderation_rate_limit(
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
    let chrono_minutes = Duration::minutes(time_window_minutes as i64);
    let since = now - chrono_minutes;
    let count = moderation_activity_repo
        .count_moderated_messages_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, message_count, time_window_minutes))
}

#[cfg(test)]
mod tests;
