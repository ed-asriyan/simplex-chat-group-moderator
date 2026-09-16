use crate::domain::moderator::ports::{
    Err, MessengerGroupId, UserCharacterActivityRepository, UserId,
};
use chrono::{DateTime, Duration, Utc};

/// Evaluates if the author wrote at least `character_count` characters in the window.
pub fn should_moderate(
    count: u32,
    character_count: u32,
    time_window_minutes: u32,
) -> Option<String> {
    if character_count > 0 && time_window_minutes > 0 && count >= character_count {
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
    let chrono_minutes = Duration::minutes(time_window_minutes as i64);
    let since = now - chrono_minutes;
    let count = activity_repo
        .sum_characters_since(group_id, user_id, since, now)
        .await?;
    Ok(should_moderate(count, character_count, time_window_minutes))
}

#[cfg(test)]
mod tests;
