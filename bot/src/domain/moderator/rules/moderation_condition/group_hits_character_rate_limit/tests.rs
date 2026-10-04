use super::*;

fn limit(character_count: u32, time_window_minutes: u32) -> GroupHitsCharacterRateLimit {
    GroupHitsCharacterRateLimit {
        character_count,
        time_window_minutes,
    }
}

fn err_of(mut condition: GroupHitsCharacterRateLimit) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

/// The group limits never had a "0 means off" state to keep, so a zero is a
/// rule that cannot match, and a window past the counters' retention would
/// quietly count less than it says.
#[test]
fn test_rejects_a_zero_count_or_a_window_out_of_range() {
    assert!(err_of(limit(0, 1)).contains("needs at least 1"));
    for window in [0, 61] {
        assert!(err_of(limit(10, window)).contains("needs a time window between 1 and 60 minutes"));
    }
}

#[test]
fn test_accepts_a_count_and_a_window_within_range() {
    for window in [1, 60] {
        let mut condition = limit(1, window);
        let unchanged = condition.clone();
        condition.normalize_and_validate().unwrap();
        assert_eq!(condition, unchanged);
    }
}

#[test]
fn test_reached_at_the_limit_not_below_it() {
    assert!(filter::should_moderate(9, 10, 1).is_none());
    assert!(filter::should_moderate(10, 10, 1).is_some());
}
