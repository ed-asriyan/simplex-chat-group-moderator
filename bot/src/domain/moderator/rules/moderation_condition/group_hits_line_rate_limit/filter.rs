use crate::domain::moderator::ports::{Err, GroupLineActivityRepository, MessengerGroupId};
use crate::domain::moderator::rules::common::rate_limit::{reached, window_start};
use chrono::{DateTime, Utc};

/// Evaluates if the whole group received at least `line_count` lines in the
/// window.
pub fn should_moderate(count: u32, line_count: u32, time_window_minutes: u32) -> Option<String> {
    if reached(count, line_count, time_window_minutes) {
        Some(format!(
            "group received {count} lines in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Queries the group-wide counter and evaluates the condition: every member's lines count.
pub async fn check(
    activity_repo: &dyn GroupLineActivityRepository,
    group_id: &MessengerGroupId,
    line_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if line_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let since = window_start(now, time_window_minutes);
    let count = activity_repo.sum_lines_since(group_id, since, now).await?;
    Ok(should_moderate(count, line_count, time_window_minutes))
}
