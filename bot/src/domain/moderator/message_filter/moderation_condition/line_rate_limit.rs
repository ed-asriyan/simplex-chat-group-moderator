use crate::domain::moderator::ports::{Err, MessengerGroupId, UserId, UserLineActivityRepository};
use chrono::{DateTime, Duration, Utc};

/// Evaluates if the author's messages took at least `line_count` lines in the
/// window.
pub fn should_moderate(count: u32, line_count: u32, time_window_minutes: u32) -> Option<String> {
    if line_count > 0 && time_window_minutes > 0 && count >= line_count {
        Some(format!(
            "author sent {count} lines in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Helper that queries the `UserLineActivityRepository` and evaluates the condition.
///
/// The wrap width is not a parameter here: each message was counted into the
/// window when it arrived, with the width configured at that moment.
pub async fn check(
    activity_repo: &dyn UserLineActivityRepository,
    group_id: &MessengerGroupId,
    user_id: &UserId,
    line_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if line_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let chrono_minutes = Duration::minutes(time_window_minutes as i64);
    let since = now - chrono_minutes;
    let count = activity_repo
        .sum_lines_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, line_count, time_window_minutes))
}

#[cfg(test)]
mod tests;
