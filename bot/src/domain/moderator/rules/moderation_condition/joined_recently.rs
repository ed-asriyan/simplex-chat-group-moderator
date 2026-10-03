use chrono::{DateTime, Duration, Utc};

/// Evaluates if the author joined the group less than `time_window_minutes`
/// before `now`.
///
/// An unknown join time (`None`) never matches: it belongs to members who were
/// already in the group when the bot joined, so they are not newcomers.
pub fn should_moderate(
    joined_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    time_window_minutes: u32,
) -> Option<String> {
    // Validation rejects 0 on save; guard anyway so a bad stored value is inert.
    if time_window_minutes == 0 {
        return None;
    }
    let elapsed = now - joined_at?;
    if elapsed < Duration::minutes(time_window_minutes as i64) {
        // A join time slightly after the message (clock skew between the two
        // records) still means "just joined", so it is clamped rather than rejected.
        let elapsed_minutes = elapsed.num_minutes().max(0);
        Some(format!(
            "author joined {elapsed_minutes} min ago (less than {time_window_minutes} min)"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
