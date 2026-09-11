use super::ModerationCondition;

fn err_of(condition: &mut ModerationCondition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

#[test]
fn test_rejects_too_many_keywords() {
    let mut condition = ModerationCondition::ContainsWords {
        keywords: (0..10_001).map(|i| format!("kw{i}")).collect(),
    };
    assert!(err_of(&mut condition).contains("Too many keywords"));
}

#[test]
fn test_rejects_too_long_keyword() {
    let mut condition = ModerationCondition::ContainsWords {
        keywords: vec!["a".repeat(101)],
    };
    assert!(err_of(&mut condition).contains("Keyword too long"));
}

#[test]
fn test_keyword_length_is_counted_in_characters_not_bytes() {
    // 100 Cyrillic characters are 200 bytes but must still be accepted.
    let mut condition = ModerationCondition::ContainsWords {
        keywords: vec!["я".repeat(100)],
    };
    condition.normalize_and_validate().unwrap();
}

#[test]
fn test_duplicate_keywords_are_collapsed_before_the_limit_applies() {
    // Well over the limit, but they collapse to a single distinct keyword.
    let mut condition = ModerationCondition::ContainsWords {
        keywords: std::iter::repeat_n("spam".to_string(), 10_001).collect(),
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        ModerationCondition::ContainsWords {
            keywords: vec!["spam".to_string()]
        }
    );
}

#[test]
fn test_keywords_are_sorted_and_empty_entries_dropped() {
    let mut condition = ModerationCondition::ContainsWords {
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
        ModerationCondition::ContainsWords {
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
fn test_rejects_too_long_listed_domain() {
    let mut condition = ModerationCondition::ContainsLinksInList {
        domains: vec![format!("{}.com", "a".repeat(100))],
    };
    assert!(err_of(&mut condition).contains("Domain too long"));
}

#[test]
fn test_rejects_too_many_domains() {
    let mut condition = ModerationCondition::ContainsLinksOutsideList {
        domains: (0..10_001).map(|i| format!("site{i}.com")).collect(),
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

    let mut in_list = ModerationCondition::ContainsLinksInList { domains: messy() };
    in_list.normalize_and_validate().unwrap();
    assert_eq!(
        in_list,
        ModerationCondition::ContainsLinksInList { domains: tidy() }
    );

    let mut outside_list = ModerationCondition::ContainsLinksOutsideList { domains: messy() };
    outside_list.normalize_and_validate().unwrap();
    assert_eq!(
        outside_list,
        ModerationCondition::ContainsLinksOutsideList { domains: tidy() }
    );

    let mut top100 = ModerationCondition::ContainsLinksOutsideTop100 { domains: messy() };
    top100.normalize_and_validate().unwrap();
    assert_eq!(
        top100,
        ModerationCondition::ContainsLinksOutsideTop100 { domains: tidy() }
    );
}

#[test]
fn test_conditions_without_list_parameters_are_left_alone() {
    for mut condition in [
        ModerationCondition::IsBlank,
        ModerationCondition::ContainsInvisibleCharacters,
        ModerationCondition::ExceedsMaxCharacters { max_characters: 1 },
        ModerationCondition::ExceedsMaxWords { max_words: 10 },
        ModerationCondition::ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 0,
        },
        ModerationCondition::AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 2,
        },
        // 0 disables these two rather than being rejected.
        ModerationCondition::AuthorHitsMessageRateLimit {
            message_count: 0,
            time_window_minutes: 0,
        },
        ModerationCondition::AuthorHitsModerationRateLimit {
            message_count: 3,
            time_window_minutes: 10,
        },
    ] {
        let unchanged = condition.clone();
        condition.normalize_and_validate().unwrap();
        assert_eq!(condition, unchanged);
    }
}

#[test]
fn test_rejects_length_conditions_with_zero_maximum() {
    // 0 used to mean "this check is off" inside the old combined condition;
    // on its own it would store a rule that can never match.
    assert!(
        err_of(&mut ModerationCondition::ExceedsMaxCharacters { max_characters: 0 })
            .contains("'Message Exceeds Max Characters' needs a maximum of at least 1")
    );
    assert!(
        err_of(&mut ModerationCondition::ExceedsMaxWords { max_words: 0 })
            .contains("'Message Exceeds Max Words' needs a maximum of at least 1")
    );
    assert!(
        err_of(&mut ModerationCondition::ExceedsMaxLines {
            max_lines: 0,
            chars_per_line: 40,
        })
        .contains("'Message Exceeds Max Lines' needs a maximum of at least 1")
    );
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

fn repeated(min_repeats: u32, min_length: u32) -> ModerationCondition {
    ModerationCondition::ContainsRepeatedSequence {
        min_repeats,
        min_length,
    }
}

#[test]
fn test_rejects_repeated_sequence_with_fewer_than_two_repeats() {
    // One occurrence is not a repetition; allowing it would match every message.
    assert!(err_of(&mut repeated(1, 1)).contains("Minimum repeats must be at least 2"));
    assert!(err_of(&mut repeated(0, 1)).contains("Minimum repeats must be at least 2"));
}

#[test]
fn test_rejects_repeated_sequence_length_out_of_range() {
    assert!(err_of(&mut repeated(5, 0)).contains("Minimum sequence length"));
    assert!(err_of(&mut repeated(5, 51)).contains("Minimum sequence length"));
}

#[test]
fn test_accepts_valid_repeated_sequence_settings() {
    for mut condition in [repeated(2, 1), repeated(5, 2), repeated(1000, 50)] {
        let unchanged = condition.clone();
        condition.normalize_and_validate().unwrap();
        assert_eq!(condition, unchanged);
    }
}

#[test]
fn test_rejects_joined_recently_with_zero_window() {
    let mut condition = ModerationCondition::AuthorJoinedRecently {
        time_window_minutes: 0,
    };
    assert!(
        err_of(&mut condition)
            .contains("'Author Joined Recently' needs a time window of at least 1 minute")
    );
}

#[test]
fn test_accepts_joined_recently_with_positive_window() {
    for time_window_minutes in [1, 60, u32::MAX] {
        let mut condition = ModerationCondition::AuthorJoinedRecently {
            time_window_minutes,
        };
        let unchanged = condition.clone();
        condition.normalize_and_validate().unwrap();
        assert_eq!(condition, unchanged);
    }
}

// ---------------------------------------------------------------------------
// Composite conditions: normalization
//
// Normalization runs bottom-up, so one pass reaches the fixpoint. These cases
// pin down each rewrite and, just as importantly, the rewrites that must *not*
// happen.
// ---------------------------------------------------------------------------

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords {
        keywords: vec![keyword.to_string()],
    }
}

fn normalized(condition: ModerationCondition) -> ModerationCondition {
    let mut condition = condition;
    condition
        .normalize_and_validate()
        .expect("condition should have been accepted");
    condition
}

#[test]
fn test_flattens_nested_composites_of_the_same_kind() {
    let condition = normalized(ModerationCondition::All {
        conditions: vec![
            ModerationCondition::All {
                conditions: vec![words("a"), words("b")],
            },
            words("c"),
        ],
    });
    assert_eq!(
        condition,
        ModerationCondition::All {
            conditions: vec![words("a"), words("b"), words("c")],
        }
    );
}

#[test]
fn test_does_not_flatten_composites_of_the_other_kind() {
    let inner = ModerationCondition::Any {
        conditions: vec![words("a"), words("b")],
    };
    let condition = normalized(ModerationCondition::All {
        conditions: vec![inner.clone(), words("c")],
    });
    assert_eq!(
        condition,
        ModerationCondition::All {
            conditions: vec![inner, words("c")],
        }
    );
}

#[test]
fn test_unwraps_a_composite_with_a_single_child() {
    let condition = normalized(ModerationCondition::All {
        conditions: vec![words("only")],
    });
    assert_eq!(condition, words("only"));
}

#[test]
fn test_drops_duplicate_siblings() {
    let condition = normalized(ModerationCondition::Any {
        conditions: vec![words("a"), words("a"), words("b")],
    });
    assert_eq!(
        condition,
        ModerationCondition::Any {
            conditions: vec![words("a"), words("b")],
        }
    );
}

#[test]
fn test_collapses_double_negation() {
    let condition = normalized(ModerationCondition::Not {
        condition: Box::new(ModerationCondition::Not {
            condition: Box::new(words("a")),
        }),
    });
    assert_eq!(condition, words("a"));
}

#[test]
fn test_drops_an_empty_composite_nested_in_another() {
    // The editor's "add" button produces an empty group the same way it
    // produces a blank list row, so this is noise rather than intent.
    let condition = normalized(ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::Any { conditions: vec![] },
            words("b"),
        ],
    });
    assert_eq!(
        condition,
        ModerationCondition::All {
            conditions: vec![words("a"), words("b")],
        }
    );
}

#[test]
fn test_collapsing_a_child_can_collapse_its_parent_in_one_pass() {
    // Any[] disappears, leaving All with one child, which unwraps, leaving Not
    // with a single meaningful child. Every rewrite happens in the same pass
    // because children are normalized before their parent.
    let condition = normalized(ModerationCondition::Not {
        condition: Box::new(ModerationCondition::All {
            conditions: vec![ModerationCondition::Any { conditions: vec![] }, words("a")],
        }),
    });
    assert_eq!(
        condition,
        ModerationCondition::Not {
            condition: Box::new(words("a")),
        }
    );
}

#[test]
fn test_normalization_is_idempotent() {
    let once = normalized(ModerationCondition::All {
        conditions: vec![
            ModerationCondition::All {
                conditions: vec![words("a"), words("a")],
            },
            ModerationCondition::Not {
                condition: Box::new(ModerationCondition::Not {
                    condition: Box::new(words("b")),
                }),
            },
        ],
    });
    assert_eq!(normalized(once.clone()), once);
}

#[test]
fn test_normalizes_the_leaves_inside_a_composite() {
    // Blank and duplicate list entries are cleaned up at any depth, not just
    // when the condition is the whole rule.
    let condition = normalized(ModerationCondition::All {
        conditions: vec![
            ModerationCondition::ContainsWords {
                keywords: vec!["b".into(), String::new(), "a".into(), "a".into()],
            },
            words("z"),
        ],
    });
    assert_eq!(
        condition,
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::ContainsWords {
                    keywords: vec!["a".into(), "b".into()],
                },
                words("z"),
            ],
        }
    );
}

#[test]
fn test_validates_the_leaves_inside_a_composite() {
    let mut condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::MatchesRegex {
            patterns: vec!["(unclosed".to_string()],
        }),
    };
    assert!(err_of(&mut condition).contains("Invalid regex pattern"));
}

#[test]
fn test_rejects_a_rule_whose_condition_collapses_to_nothing() {
    // Dropping the rule silently would leave the owner believing it exists.
    let mut condition = ModerationCondition::All {
        conditions: vec![ModerationCondition::Any { conditions: vec![] }],
    };
    assert!(err_of(&mut condition).contains("empty condition"));
}

#[test]
fn test_rejects_a_tree_that_is_too_deep() {
    let mut condition = words("a");
    for _ in 0..super::MAX_CONDITION_DEPTH {
        condition = ModerationCondition::Not {
            condition: Box::new(ModerationCondition::Any {
                conditions: vec![condition, words("filler")],
            }),
        };
    }
    assert!(err_of(&mut condition).contains("too deep"));
}

#[test]
fn test_rejects_a_tree_with_too_many_nodes() {
    let mut condition = ModerationCondition::Any {
        conditions: (0..super::MAX_CONDITION_NODES)
            .map(|i| words(&format!("kw{i}")))
            .collect(),
    };
    assert!(err_of(&mut condition).contains("Too many conditions"));
}

#[test]
fn test_rejects_moderation_rate_limit_under_a_negation() {
    // The pre-pass answers "is this message moderated by something else" by
    // pinning these nodes to "no match". Under a negation, pinning would make
    // the enclosing rule fire, so the answer would depend on itself.
    let mut condition = ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::Not {
                condition: Box::new(ModerationCondition::AuthorHitsModerationRateLimit {
                    message_count: 2,
                    time_window_minutes: 5,
                }),
            },
        ],
    };
    let err = err_of(&mut condition);
    assert!(err.contains("'Author Hits Moderation Rate Limit' cannot be placed"));
    assert!(err.contains("under a 'Not'"));
}

#[test]
fn test_allows_moderation_rate_limit_outside_a_negation() {
    normalized(ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::AuthorHitsModerationRateLimit {
                message_count: 2,
                time_window_minutes: 5,
            },
        ],
    });
}

// ---------------------------------------------------------------------------
// Composite conditions: tree navigation
// ---------------------------------------------------------------------------

#[test]
fn test_activity_windows_are_found_at_any_depth() {
    let condition = ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::AuthorHitsMessageRateLimit {
                        message_count: 3,
                        time_window_minutes: 7,
                    },
                    ModerationCondition::Not {
                        condition: Box::new(ModerationCondition::AuthorHitsMessageRateLimit {
                            message_count: 9,
                            time_window_minutes: 30,
                        }),
                    },
                ],
            },
        ],
    };
    assert_eq!(condition.max_message_rate_limit_window(), Some(30));
    assert_eq!(condition.max_moderation_rate_limit_window(), None);
    assert!(!condition.contains_moderation_rate_limit());
}

#[test]
fn test_zero_windows_are_ignored() {
    let condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::AuthorHitsMessageRateLimit {
            message_count: 3,
            time_window_minutes: 0,
        }),
    };
    assert_eq!(condition.max_message_rate_limit_window(), None);
}

// ---------------------------------------------------------------------------
// Describing a condition (what a `Not` reports when it matches)
// ---------------------------------------------------------------------------

#[test]
fn test_descriptions_state_what_is_detected_without_judging_it() {
    let described = [
        words("a"),
        ModerationCondition::MatchesExactMessage {
            messages: vec![],
            case_sensitive: false,
        },
        ModerationCondition::ContainsLinksInList { domains: vec![] },
        ModerationCondition::ContainsLinksOutsideList { domains: vec![] },
        ModerationCondition::ContainsLinksOutsideTop100 { domains: vec![] },
        ModerationCondition::IsBlank,
        ModerationCondition::ContainsInvisibleCharacters,
        ModerationCondition::ExceedsMaxCharacters { max_characters: 10 },
        ModerationCondition::ExceedsMaxWords { max_words: 10 },
        ModerationCondition::ExceedsMaxLines {
            max_lines: 10,
            chars_per_line: 40,
        },
    ]
    .map(|condition| condition.describe());
    for description in described {
        for judgement in [
            "banned",
            "forbidden",
            "blacklist",
            "allowed",
            "approved",
            "safe",
        ] {
            assert!(
                !description.contains(judgement),
                "'{description}' contains '{judgement}'"
            );
        }
    }
}

#[test]
fn test_descriptions_carry_the_thresholds() {
    assert_eq!(
        ModerationCondition::ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 40,
        }
        .describe(),
        "has more than 5 lines"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 2,
        }
        .describe(),
        "author sent at least 5 messages in 2 min"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsModerationRateLimit {
            message_count: 3,
            time_window_minutes: 60,
        }
        .describe(),
        "author had at least 3 messages moderated in 60 min"
    );
}

#[test]
fn test_parameterless_conditions_round_trip_through_json() {
    // They carry nothing but their tag, which is exactly what the editor sends.
    for (condition, json) in [
        (ModerationCondition::IsBlank, r#"{"type":"IsBlank"}"#),
        (
            ModerationCondition::ContainsInvisibleCharacters,
            r#"{"type":"ContainsInvisibleCharacters"}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&condition).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<ModerationCondition>(json).unwrap(),
            condition
        );
    }
}
