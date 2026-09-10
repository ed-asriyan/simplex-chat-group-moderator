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

#[test]
fn test_repeated_calls_are_consistent() {
    // The compiled-pattern cache must not change what a repeated call answers.
    let patterns = vec![r"\d{3}".to_string()];
    for _ in 0..3 {
        assert_eq!(
            should_moderate("code 123", &patterns),
            Some(r"\d{3}".to_string())
        );
        assert!(should_moderate("no digits", &patterns).is_none());
    }
}

#[test]
fn test_many_distinct_patterns() {
    // Cached patterns stay correct even when many distinct ones are loaded.
    // (Overflow at 10k limit is rare and handled via clear(), not tested here.)
    let patterns: Vec<String> = (0..1000).map(|i| format!("^unique{i}$")).collect();
    for pattern in &patterns {
        assert!(should_moderate(&pattern[1..pattern.len() - 1], &[pattern.clone()]).is_some());
    }
    assert_eq!(
        should_moderate("unique42", &patterns),
        Some("^unique42$".to_string())
    );
}
