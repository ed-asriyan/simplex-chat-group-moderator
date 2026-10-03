use super::super::conditions::{
    AuthorHitsModerationRateLimit, ContainsWords, FlaggedByOmniModeration, MatchesRegex,
};
use super::ModerationCondition;
use crate::domain::moderator::ports::{CategoryTrigger, OpenAiCategoryTriggers};

fn err_of(condition: &mut ModerationCondition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

// ---------------------------------------------------------------------------
// Composite conditions: normalization
//
// Normalization runs bottom-up, so one pass reaches the fixpoint. These cases
// pin down each rewrite and, just as importantly, the rewrites that must *not*
// happen.
// ---------------------------------------------------------------------------

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords(ContainsWords {
        keywords: vec![keyword.to_string()],
    })
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
            ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["b".into(), String::new(), "a".into(), "a".into()],
            }),
            words("z"),
        ],
    });
    assert_eq!(
        condition,
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["a".into(), "b".into()],
                }),
                words("z"),
            ],
        }
    );
}

#[test]
fn test_validates_the_leaves_inside_a_composite() {
    let mut condition = ModerationCondition::Not {
        condition: Box::new(ModerationCondition::MatchesRegex(MatchesRegex {
            patterns: vec!["(unclosed".to_string()],
        })),
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
                condition: Box::new(ModerationCondition::AuthorHitsModerationRateLimit(
                    AuthorHitsModerationRateLimit {
                        message_count: 2,
                        time_window_minutes: 5,
                    },
                )),
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
            ModerationCondition::AuthorHitsModerationRateLimit(AuthorHitsModerationRateLimit {
                message_count: 2,
                time_window_minutes: 5,
            }),
        ],
    });
}

#[test]
fn test_openai_condition_may_sit_anywhere_in_the_tree_even_under_not() {
    // The tree knows nothing about its leaves: no placement is special.
    let mut condition = ModerationCondition::Any {
        conditions: vec![
            words("spam"),
            ModerationCondition::Not {
                condition: Box::new(ModerationCondition::FlaggedByOmniModeration(
                    FlaggedByOmniModeration {
                        api_key: "sk-proj-abc".to_string(),
                        triggers: OpenAiCategoryTriggers {
                            hate: CategoryTrigger::OpenAiDecides,
                            ..Default::default()
                        },
                        retry: Default::default(),
                    },
                )),
            },
        ],
    };
    condition.normalize_and_validate().unwrap();
}
