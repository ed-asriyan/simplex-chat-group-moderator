use crate::domain::moderator::ports::{
    Err, GroupCharacterActivityRepository, MessengerGroupId, UserCharacterActivityRepository,
    UserId,
};
use crate::domain::moderator::rules::common::rate_limit::{reached, window_start};
use chrono::{DateTime, Utc};

/// Evaluates if the author wrote at least `character_count` characters in the window.
pub fn should_moderate(
    count: u32,
    character_count: u32,
    time_window_minutes: u32,
) -> Option<String> {
    if reached(count, character_count, time_window_minutes) {
        Some(format!(
            "author sent {count} characters in {time_window_minutes} min"
        ))
    } else {
        None
    }
}

/// Helper that queries the `UserCharacterActivityRepository` and evaluates the condition.
pub async fn check(
    activity_repo: &dyn UserCharacterActivityRepository,
    group_id: &MessengerGroupId,
    user_id: &UserId,
    character_count: u32,
    time_window_minutes: u32,
    now: DateTime<Utc>,
) -> Result<Option<String>, Err> {
    if character_count == 0 || time_window_minutes == 0 {
        return Ok(None);
    }
    let since = window_start(now, time_window_minutes);
    let count = activity_repo
        .sum_characters_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, character_count, time_window_minutes))
}

/// Evaluates if the whole group received at least `character_count` characters in the
/// window.
pub fn group_should_moderate(
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

/// The group-wide counterpart of [`check`]: every member's characters count.
pub async fn check_group(
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
    Ok(group_should_moderate(
        count,
        character_count,
        time_window_minutes,
    ))
}

#[cfg(test)]
mod tests;
