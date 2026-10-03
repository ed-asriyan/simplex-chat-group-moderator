use crate::domain::moderator::ports::{
    Err, GroupMessageActivityRepository, MessengerGroupId, UserId, UserMessageActivityRepository,
};
use chrono::{DateTime, Duration, Utc};

/// Evaluates if the author sent at least `message_count` messages in the window.
pub fn should_moderate(count: u32, message_count: u32, time_window_minutes: u32) -> Option<String> {
    if message_count > 0 && time_window_minutes > 0 && count >= message_count {
        Some(format!(
            "author sent {count} messages in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Helper that queries the `UserMessageActivityRepository` and evaluates the condition.
pub async fn check(
    activity_repo: &dyn UserMessageActivityRepository,
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
    let count = activity_repo
        .count_messages_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, message_count, time_window_minutes))
}

/// Evaluates if the whole group received at least `message_count` messages in the
/// window.
pub fn group_should_moderate(
    count: u32,
    message_count: u32,
    time_window_minutes: u32,
) -> Option<String> {
    if message_count > 0 && time_window_minutes > 0 && count >= message_count {
        Some(format!(
            "group received {count} messages in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// The group-wide counterpart of [`check`]: every member's messages count.
pub async fn check_group(
    activity_repo: &dyn GroupMessageActivityRepository,
    group_id: &MessengerGroupId,
    message_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if message_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let since = now - Duration::minutes(time_window_minutes as i64);
    let count = activity_repo
        .count_messages_since(group_id, since, now)
        .await?;
    Ok(group_should_moderate(
        count,
        message_count,
        time_window_minutes,
    ))
}

#[cfg(test)]
mod tests;
