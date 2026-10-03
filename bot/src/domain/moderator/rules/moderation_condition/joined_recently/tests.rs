use super::should_moderate;
use chrono::{DateTime, Duration, TimeZone, Utc};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
}

#[test]
fn test_matches_author_who_joined_within_window() {
    let joined_at = now() - Duration::minutes(3);
    assert_eq!(
        should_moderate(Some(joined_at), now(), 10),
        Some("author joined 3 min ago (less than 10 min)".to_string())
    );
}

#[test]
fn test_does_not_match_author_who_joined_before_window() {
    let joined_at = now() - Duration::minutes(11);
    assert!(should_moderate(Some(joined_at), now(), 10).is_none());
}

#[test]
fn test_window_boundary_is_exclusive() {
    // "Less than 10 minutes ago": exactly 10 minutes no longer counts.
    let at_boundary = now() - Duration::minutes(10);
    assert!(should_moderate(Some(at_boundary), now(), 10).is_none());

    let just_inside = at_boundary + Duration::seconds(1);
    assert_eq!(
        should_moderate(Some(just_inside), now(), 10),
        Some("author joined 9 min ago (less than 10 min)".to_string())
    );
}

#[test]
fn test_unknown_join_time_never_matches() {
    // Members who were in the group before the bot have no known join time.
    assert!(should_moderate(None, now(), 10).is_none());
    assert!(should_moderate(None, now(), u32::MAX).is_none());
}

#[test]
fn test_zero_window_never_matches() {
    assert!(should_moderate(Some(now()), now(), 0).is_none());
}

#[test]
fn test_join_time_after_message_counts_as_just_joined() {
    let joined_at = now() + Duration::seconds(5);
    assert_eq!(
        should_moderate(Some(joined_at), now(), 1),
        Some("author joined 0 min ago (less than 1 min)".to_string())
    );
}

#[test]
fn test_huge_window_does_not_overflow() {
    let joined_at = now() - Duration::days(365 * 100);
    assert!(should_moderate(Some(joined_at), now(), u32::MAX).is_some());
}
