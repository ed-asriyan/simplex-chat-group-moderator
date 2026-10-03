use super::*;
use super::filter::should_moderate;

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
    // Unlike the keywords filter, raw regex does not fold look-alikes,
    // compatibility letter variants, or upside-down text.
    let patterns = vec!["spam".to_string()];
    assert!(should_moderate("sp4m", &patterns).is_none());
    assert!(should_moderate("𝐬𝐩𝐚𝐦", &patterns).is_none());
    assert!(should_moderate("ɯɐds", &patterns).is_none());
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

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

fn err_of(condition: &mut impl Condition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

#[test]
fn test_rejects_regex_pattern_that_does_not_compile() {
    let mut condition = MatchesRegex {
        patterns: vec!["(unclosed".to_string()],
    };
    assert!(err_of(&mut condition).contains("Invalid regex pattern"));
}

#[test]
fn test_rejects_regex_pattern_using_unsupported_syntax() {
    // The regex crate has no backreferences; the owner must find out when they
    // save the rule, not silently never match.
    let mut condition = MatchesRegex {
        patterns: vec![r"(.)\1{5,}".to_string()],
    };
    assert!(err_of(&mut condition).contains("Invalid regex pattern"));
}

#[test]
fn test_rejects_too_many_regex_patterns() {
    let mut condition = MatchesRegex {
        patterns: (0..101).map(|i| format!("pattern{i}")).collect(),
    };
    assert!(err_of(&mut condition).contains("Too many regex patterns"));
}

#[test]
fn test_rejects_too_long_regex_pattern() {
    let mut condition = MatchesRegex {
        patterns: vec!["a".repeat(201)],
    };
    assert!(err_of(&mut condition).contains("Regex pattern too long"));
}

#[test]
fn test_regex_patterns_are_normalized() {
    let mut condition = MatchesRegex {
        patterns: vec![
            r"\d{3}".to_string(),
            String::new(),
            r" {5,}".to_string(),
            r"\d{3}".to_string(),
        ],
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        MatchesRegex {
            patterns: vec![r" {5,}".to_string(), r"\d{3}".to_string()]
        }
    );
}

#[test]
fn test_accepts_valid_regex_patterns() {
    let mut condition = MatchesRegex {
        patterns: vec![r"(?i)free\s+crypto".to_string(), r" {5,}".to_string()],
    };
    condition.normalize_and_validate().unwrap();
}
