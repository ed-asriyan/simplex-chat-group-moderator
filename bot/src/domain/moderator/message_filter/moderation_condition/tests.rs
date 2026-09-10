use super::ModerationCondition;

fn err_of(condition: &mut ModerationCondition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

#[test]
fn test_rejects_too_many_keywords() {
    let mut condition = ModerationCondition::ContainsBannedWords {
        keywords: (0..10_001).map(|i| format!("kw{i}")).collect(),
    };
    assert!(err_of(&mut condition).contains("Too many keywords"));
}

#[test]
fn test_rejects_too_long_keyword() {
    let mut condition = ModerationCondition::ContainsBannedWords {
        keywords: vec!["a".repeat(101)],
    };
    assert!(err_of(&mut condition).contains("Keyword too long"));
}

#[test]
fn test_keyword_length_is_counted_in_characters_not_bytes() {
    // 100 Cyrillic characters are 200 bytes but must still be accepted.
    let mut condition = ModerationCondition::ContainsBannedWords {
        keywords: vec!["я".repeat(100)],
    };
    condition.normalize_and_validate().unwrap();
}

#[test]
fn test_duplicate_keywords_are_collapsed_before_the_limit_applies() {
    // Well over the limit, but they collapse to a single distinct keyword.
    let mut condition = ModerationCondition::ContainsBannedWords {
        keywords: std::iter::repeat_n("spam".to_string(), 10_001).collect(),
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        ModerationCondition::ContainsBannedWords {
            keywords: vec!["spam".to_string()]
        }
    );
}

#[test]
fn test_keywords_are_sorted_and_empty_entries_dropped() {
    let mut condition = ModerationCondition::ContainsBannedWords {
        keywords: vec![
            "beta".to_string(),
            String::new(),
            "alpha".to_string(),
            "beta".to_string(),
        ],
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        ModerationCondition::ContainsBannedWords {
            keywords: vec!["alpha".to_string(), "beta".to_string()]
        }
    );
}

#[test]
fn test_rejects_too_many_messages() {
    let mut condition = ModerationCondition::MatchesExactMessage {
        messages: (0..10_001).map(|i| format!("msg{i}")).collect(),
        case_sensitive: false,
    };
    assert!(err_of(&mut condition).contains("Too many messages"));
}

#[test]
fn test_rejects_too_long_message() {
    let mut condition = ModerationCondition::MatchesExactMessage {
        messages: vec!["a".repeat(1001)],
        case_sensitive: false,
    };
    assert!(err_of(&mut condition).contains("Message too long"));
}

#[test]
fn test_message_case_sensitivity_flag_is_preserved() {
    let mut condition = ModerationCondition::MatchesExactMessage {
        messages: vec!["b".to_string(), "a".to_string()],
        case_sensitive: true,
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        ModerationCondition::MatchesExactMessage {
            messages: vec!["a".to_string(), "b".to_string()],
            case_sensitive: true,
        }
    );
}

#[test]
fn test_rejects_too_long_blocked_domain() {
    let mut condition = ModerationCondition::ContainsLinksToForbiddenWebsites {
        blocked: vec![format!("{}.com", "a".repeat(100))],
    };
    assert!(err_of(&mut condition).contains("Domain too long"));
}

#[test]
fn test_rejects_too_many_allowed_domains() {
    let mut condition = ModerationCondition::ContainsLinksOutsideAllowedList {
        allowed: (0..10_001).map(|i| format!("site{i}.com")).collect(),
    };
    assert!(err_of(&mut condition).contains("Too many domains"));
}

#[test]
fn test_every_domain_list_is_normalized() {
    let messy = || {
        vec![
            "b.com".to_string(),
            String::new(),
            "a.com".to_string(),
            "b.com".to_string(),
        ]
    };
    let tidy = || vec!["a.com".to_string(), "b.com".to_string()];

    let mut blocked = ModerationCondition::ContainsLinksToForbiddenWebsites { blocked: messy() };
    blocked.normalize_and_validate().unwrap();
    assert_eq!(
        blocked,
        ModerationCondition::ContainsLinksToForbiddenWebsites { blocked: tidy() }
    );

    let mut allowed = ModerationCondition::ContainsLinksOutsideAllowedList { allowed: messy() };
    allowed.normalize_and_validate().unwrap();
    assert_eq!(
        allowed,
        ModerationCondition::ContainsLinksOutsideAllowedList { allowed: tidy() }
    );

    let mut top100 = ModerationCondition::ContainsLinksOutsideTop100 { allowed: messy() };
    top100.normalize_and_validate().unwrap();
    assert_eq!(
        top100,
        ModerationCondition::ContainsLinksOutsideTop100 { allowed: tidy() }
    );
}

#[test]
fn test_conditions_without_list_parameters_are_left_alone() {
    let mut flooding = ModerationCondition::FloodsChatOrExceedsLimits {
        max_characters: 0,
        max_words: 0,
        max_lines: 0,
        chars_per_line: 40,
        disallow_empty_messages: true,
        disallow_invisible_chars: false,
    };
    let unchanged = flooding.clone();
    flooding.normalize_and_validate().unwrap();
    assert_eq!(flooding, unchanged);

    let mut rate_limit = ModerationCondition::UserExceedsMessagesRateLimit {
        message_count: 5,
        time_window_minutes: 2,
    };
    let unchanged = rate_limit.clone();
    rate_limit.normalize_and_validate().unwrap();
    assert_eq!(rate_limit, unchanged);

    let mut moderation_rate_limit = ModerationCondition::UserExceedsModerationRateLimit {
        message_count: 3,
        time_window_minutes: 10,
    };
    let unchanged = moderation_rate_limit.clone();
    moderation_rate_limit.normalize_and_validate().unwrap();
    assert_eq!(moderation_rate_limit, unchanged);
}

#[test]
fn test_rejects_regex_pattern_that_does_not_compile() {
    let mut condition = ModerationCondition::MatchesRegex {
        patterns: vec!["(unclosed".to_string()],
    };
    assert!(err_of(&mut condition).contains("Invalid regex pattern"));
}

#[test]
fn test_rejects_regex_pattern_using_unsupported_syntax() {
    // The regex crate has no backreferences; the owner must find out when they
    // save the rule, not silently never match.
    let mut condition = ModerationCondition::MatchesRegex {
        patterns: vec![r"(.)\1{5,}".to_string()],
    };
    assert!(err_of(&mut condition).contains("Invalid regex pattern"));
}

#[test]
fn test_rejects_too_many_regex_patterns() {
    let mut condition = ModerationCondition::MatchesRegex {
        patterns: (0..101).map(|i| format!("pattern{i}")).collect(),
    };
    assert!(err_of(&mut condition).contains("Too many regex patterns"));
}

#[test]
fn test_rejects_too_long_regex_pattern() {
    let mut condition = ModerationCondition::MatchesRegex {
        patterns: vec!["a".repeat(201)],
    };
    assert!(err_of(&mut condition).contains("Regex pattern too long"));
}

#[test]
fn test_regex_patterns_are_normalized() {
    let mut condition = ModerationCondition::MatchesRegex {
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
        ModerationCondition::MatchesRegex {
            patterns: vec![r" {5,}".to_string(), r"\d{3}".to_string()]
        }
    );
}

#[test]
fn test_accepts_valid_regex_patterns() {
    let mut condition = ModerationCondition::MatchesRegex {
        patterns: vec![r"(?i)free\s+crypto".to_string(), r" {5,}".to_string()],
    };
    condition.normalize_and_validate().unwrap();
}
