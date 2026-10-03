use crate::domain::moderator::ports::{Err, GroupCharacterActivityRepository, MessengerGroupId};
use crate::domain::moderator::rules::common::rate_limit::{reached, window_start};
use chrono::{DateTime, Utc};

/// Evaluates if the whole group received at least `character_count` characters in the
/// window.
pub fn should_moderate(
    count: u32,
    character_count: u32,
    time_window_minutes: u32,
) -> Option<String> {
    if reached(count, character_count, time_window_minutes) {
        Some(format!(
            "group received {count} characters in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Queries the group-wide counter and evaluates the condition: every member's characters count.
pub async fn check(
    activity_repo: &dyn GroupCharacterActivityRepository,
    group_id: &MessengerGroupId,
    character_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if character_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let since = window_start(now, time_window_minutes);
    let count = activity_repo
        .sum_characters_since(group_id, since, now)
        .await?;
    Ok(should_moderate(
        count,
        character_count,
        time_window_minutes,
    ))
}
