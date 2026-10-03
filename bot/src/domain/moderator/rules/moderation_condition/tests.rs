use super::ModerationCondition;
use crate::domain::moderator::ports::conditions::{
    AuthorHitsCharacterRateLimit, AuthorHitsLineRateLimit, AuthorHitsMessageRateLimit,
    AuthorHitsModerationRateLimit, ContainsFile, ContainsImage, ContainsInvisibleCharacters,
    ContainsLinksInList, ContainsLinksOutsideList, ContainsLinksOutsideTop100, ContainsVideo,
    ContainsVoiceMessage, ContainsWords, ExceedsMaxCharacters, ExceedsMaxLines, ExceedsMaxWords,
    FlaggedByOpenRouterInstruction, IsBlank, MatchesExactMessage,
};

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords(ContainsWords {
        keywords: vec![keyword.to_string()],
    })
}

#[test]
fn test_conditions_without_list_parameters_are_left_alone() {
    for mut condition in [
        ModerationCondition::IsBlank(IsBlank {}),
        ModerationCondition::ContainsInvisibleCharacters(ContainsInvisibleCharacters {}),
        ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters { max_characters: 1 }),
        ModerationCondition::ExceedsMaxWords(ExceedsMaxWords { max_words: 10 }),
        ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 0,
        }),
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 2,
        }),
        // 0 disables these two rather than being rejected.
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 0,
            time_window_minutes: 0,
        }),
        ModerationCondition::AuthorHitsModerationRateLimit(AuthorHitsModerationRateLimit {
            message_count: 3,
            time_window_minutes: 10,
        }),
    ] {
        let unchanged = condition.clone();
        condition.normalize_and_validate().unwrap();
        assert_eq!(condition, unchanged);
    }
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
                    ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                        message_count: 3,
                        time_window_minutes: 7,
                    }),
                    ModerationCondition::Not {
                        condition: Box::new(ModerationCondition::AuthorHitsMessageRateLimit(
                            AuthorHitsMessageRateLimit {
                                message_count: 9,
                                time_window_minutes: 30,
                            },
                        )),
                    },
                ],
            },
        ],
    };
    assert_eq!(condition.needs().author_messages, Some(30));
    assert_eq!(condition.needs().author_moderations, None);
    assert!(!condition.depends_on_other_rules());
}

#[test]
fn test_character_windows_are_found_at_any_depth() {
    let condition = ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::AuthorHitsCharacterRateLimit(
                        AuthorHitsCharacterRateLimit {
                            character_count: 300,
                            time_window_minutes: 7,
                        },
                    ),
                    ModerationCondition::Not {
                        condition: Box::new(ModerationCondition::AuthorHitsCharacterRateLimit(
                            AuthorHitsCharacterRateLimit {
                                character_count: 5000,
                                time_window_minutes: 30,
                            },
                        )),
                    },
                ],
            },
        ],
    };
    assert_eq!(condition.needs().author_characters, Some(30));
    // The two rate limits are separate logs, so neither window answers for the
    // other: a tree full of character limits asks for no message tracking.
    assert_eq!(condition.needs().author_messages, None);
    assert_eq!(condition.needs().author_moderations, None);
}

#[test]
fn test_line_windows_and_wrap_width_are_found_at_any_depth() {
    let condition = ModerationCondition::All {
        conditions: vec![
            words("a"),
            ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                        line_count: 30,
                        time_window_minutes: 7,
                        chars_per_line: 40,
                    }),
                    ModerationCondition::Not {
                        condition: Box::new(ModerationCondition::AuthorHitsLineRateLimit(
                            AuthorHitsLineRateLimit {
                                line_count: 100,
                                time_window_minutes: 30,
                                chars_per_line: 80,
                            },
                        )),
                    },
                ],
            },
        ],
    };
    assert_eq!(
        condition
            .needs()
            .author_lines
            .map(|l| l.time_window_minutes),
        Some(30)
    );
    // One counter serves the group, so the widest width wins.
    assert_eq!(
        condition.needs().author_lines.map(|l| l.chars_per_line),
        Some(80)
    );
    // Lines are their own log: no other tracking is asked for.
    assert_eq!(condition.needs().author_messages, None);
    assert_eq!(condition.needs().author_characters, None);
    assert_eq!(condition.needs().author_moderations, None);
}

/// 0 means "no wrapping", which counts fewer lines than any width does, so it
/// is the widest setting of all.
#[test]
fn test_wrap_width_zero_beats_every_width() {
    let condition = ModerationCondition::Any {
        conditions: vec![
            ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                line_count: 30,
                time_window_minutes: 5,
                chars_per_line: 40,
            }),
            ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                line_count: 10,
                time_window_minutes: 5,
                chars_per_line: 0,
            }),
        ],
    };
    assert_eq!(
        condition.needs().author_lines.map(|l| l.chars_per_line),
        Some(0)
    );
}

#[test]
fn test_zero_line_windows_are_ignored() {
    let condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::AuthorHitsLineRateLimit(
            AuthorHitsLineRateLimit {
                line_count: 30,
                time_window_minutes: 0,
                chars_per_line: 40,
            },
        )),
    };
    assert_eq!(
        condition
            .needs()
            .author_lines
            .map(|l| l.time_window_minutes),
        None
    );
    // A disabled condition asks for no counting either.
    assert_eq!(
        condition.needs().author_lines.map(|l| l.chars_per_line),
        None
    );
}

#[test]
fn test_zero_character_windows_are_ignored() {
    let condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::AuthorHitsCharacterRateLimit(
            AuthorHitsCharacterRateLimit {
                character_count: 300,
                time_window_minutes: 0,
            },
        )),
    };
    assert_eq!(condition.needs().author_characters, None);
}

#[test]
fn test_zero_windows_are_ignored() {
    let condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::AuthorHitsMessageRateLimit(
            AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 0,
            },
        )),
    };
    assert_eq!(condition.needs().author_messages, None);
}

// ---------------------------------------------------------------------------
// Describing a condition (what a `Not` reports when it matches)
// ---------------------------------------------------------------------------

#[test]
fn test_descriptions_state_what_is_detected_without_judging_it() {
    let described = [
        words("a"),
        ModerationCondition::MatchesExactMessage(MatchesExactMessage {
            messages: vec![],
            case_sensitive: false,
        }),
        ModerationCondition::ContainsLinksInList(ContainsLinksInList { domains: vec![] }),
        ModerationCondition::ContainsLinksOutsideList(ContainsLinksOutsideList { domains: vec![] }),
        ModerationCondition::ContainsLinksOutsideTop100(ContainsLinksOutsideTop100 {
            domains: vec![],
        }),
        ModerationCondition::IsBlank(IsBlank {}),
        ModerationCondition::ContainsInvisibleCharacters(ContainsInvisibleCharacters {}),
        ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters { max_characters: 10 }),
        ModerationCondition::ExceedsMaxWords(ExceedsMaxWords { max_words: 10 }),
        ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
            max_lines: 10,
            chars_per_line: 40,
        }),
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
        ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 40,
        })
        .describe(),
        "has more than 5 lines"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 2,
        })
        .describe(),
        "author sent at least 5 messages in 2 min"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsCharacterRateLimit(AuthorHitsCharacterRateLimit {
            character_count: 2000,
            time_window_minutes: 5,
        })
        .describe(),
        "author sent at least 2000 characters in 5 min"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
            line_count: 30,
            time_window_minutes: 5,
            chars_per_line: 40,
        })
        .describe(),
        "author sent at least 30 lines in 5 min"
    );
    assert_eq!(
        ModerationCondition::AuthorHitsModerationRateLimit(AuthorHitsModerationRateLimit {
            message_count: 3,
            time_window_minutes: 60,
        })
        .describe(),
        "author had at least 3 messages moderated in 60 min"
    );
}

#[test]
fn test_parameterless_conditions_round_trip_through_json() {
    // They carry nothing but their tag, which is exactly what the editor sends.
    for (condition, json) in [
        (
            ModerationCondition::IsBlank(IsBlank {}),
            r#"{"type":"IsBlank"}"#,
        ),
        (
            ModerationCondition::ContainsInvisibleCharacters(ContainsInvisibleCharacters {}),
            r#"{"type":"ContainsInvisibleCharacters"}"#,
        ),
        (
            ModerationCondition::ContainsImage(ContainsImage {}),
            r#"{"type":"ContainsImage"}"#,
        ),
        (
            ModerationCondition::ContainsVideo(ContainsVideo {}),
            r#"{"type":"ContainsVideo"}"#,
        ),
        (
            ModerationCondition::ContainsVoiceMessage(ContainsVoiceMessage {}),
            r#"{"type":"ContainsVoiceMessage"}"#,
        ),
        (
            ModerationCondition::ContainsFile(ContainsFile {}),
            r#"{"type":"ContainsFile"}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&condition).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<ModerationCondition>(json).unwrap(),
            condition
        );
    }
}

// ---------------------------------------------------------------------------
// FlaggedByOpenRouterInstruction
// ---------------------------------------------------------------------------

fn instructed(api_key: &str, model: &str, instruction: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOpenRouterInstruction(FlaggedByOpenRouterInstruction {
        retry: Default::default(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        instruction: instruction.to_string(),
        context_messages: 0,
    })
}

#[test]
fn test_history_kept_for_openrouter_looks_through_the_whole_tree() {
    let with_context = |n| {
        let mut condition = instructed("sk-proj-abc", "openai/gpt-4o-mini", "Block ads.");
        if let ModerationCondition::FlaggedByOpenRouterInstruction(
            FlaggedByOpenRouterInstruction {
                context_messages, ..
            },
        ) = &mut condition
        {
            *context_messages = n;
        }
        condition
    };
    let tree = ModerationCondition::Any {
        conditions: vec![
            with_context(2),
            ModerationCondition::Not {
                condition: Box::new(with_context(5)),
            },
        ],
    };
    assert_eq!(tree.needs().history, Some(5));
    assert_eq!(with_context(0).needs().history, None);
    assert_eq!(
        ModerationCondition::IsBlank(IsBlank {}).needs().history,
        None
    );
}
