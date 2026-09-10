use super::should_moderate;

#[test]
fn test_matches_pattern() {
    let patterns = vec![r"\d{3}-\d{4}".to_string()];
    assert_eq!(
        should_moderate("call 555-1234 now", &patterns),
        Some(r"\d{3}-\d{4}".to_string())
    );
    assert!(should_moderate("no numbers here", &patterns).is_none());
}

#[test]
fn test_matches_repeated_spaces() {
    let patterns = vec![r" {5,}".to_string()];
    assert_eq!(
        should_moderate("hello     world", &patterns),
        Some(r" {5,}".to_string())
    );
    assert!(should_moderate("hello world", &patterns).is_none());
}

#[test]
fn test_ignores_normalization_tricks_that_keywords_would_catch() {
    // Unlike the keywords filter, raw regex does not fold look-alikes.
    let patterns = vec!["spam".to_string()];
    assert!(should_moderate("sp4m", &patterns).is_none());
    assert!(should_moderate("this is spam", &patterns).is_some());
}

#[test]
fn test_first_matching_pattern_wins() {
    let patterns = vec!["foo".to_string(), "bar".to_string()];
    assert_eq!(
        should_moderate("this has bar in it", &patterns),
        Some("bar".to_string())
    );
}

#[test]
fn test_invalid_pattern_is_skipped_not_panicking() {
    let patterns = vec!["(unclosed".to_string(), "ok".to_string()];
    assert_eq!(
        should_moderate("this is ok", &patterns),
        Some("ok".to_string())
    );
}

#[test]
fn test_no_patterns_never_matches() {
    assert!(should_moderate("anything", &[]).is_none());
}
