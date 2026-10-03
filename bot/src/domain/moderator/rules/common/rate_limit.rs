//! What every rate limit condition shares, the author's and the group's alike:
//! how long a window may be, where it starts, and when a count reaches its
//! limit.

use crate::domain::moderator::ports::Err;
use chrono::{DateTime, Duration, Utc};

/// Longest time window a rate limit may count over: the activity counters keep
/// nothing longer, so a longer window would quietly count less than it says.
pub const MAX_RATE_LIMIT_WINDOW_MINUTES: u32 = 60;

/// Checked when an owner saves a rate limit that has no "disabled" state.
pub fn check_window(time_window_minutes: u32, title: &str) -> Result<(), Err> {
    if !(1..=MAX_RATE_LIMIT_WINDOW_MINUTES).contains(&time_window_minutes) {
        return Err(format!(
            "'{title}' needs a time window between 1 and {MAX_RATE_LIMIT_WINDOW_MINUTES} minutes, got {time_window_minutes}"
        )
        .into());
    }
    Ok(())
}

/// The start of a window `time_window_minutes` long that ends `now`.
pub fn window_start(now: DateTime<Utc>, time_window_minutes: u32) -> DateTime<Utc> {
    now - Duration::minutes(time_window_minutes as i64)
}

/// Whether `count` reaches `limit`. A limit or a window of 0 never does: the
/// author's limits read 0 as "disabled".
pub fn reached(count: u32, limit: u32, time_window_minutes: u32) -> bool {
    limit > 0 && time_window_minutes > 0 && count >= limit
}
