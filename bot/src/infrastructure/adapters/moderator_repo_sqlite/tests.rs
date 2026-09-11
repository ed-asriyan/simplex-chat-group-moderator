use super::*;
use crate::infrastructure::migrations;

#[tokio::test]
async fn test_delete_group_data_removes_all_conditions_and_actions() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let messenger_group_id = 999;
    let owner_id = 123;
    let group_id = repo
        .save_owner(&messenger_group_id, "Test Group", &owner_id)
        .await
        .unwrap();

    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["word1".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::MatchesExactMessage {
                messages: vec!["msg1".to_string()],
                case_sensitive: true,
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::MatchesRegex {
                patterns: vec![r" {5,}".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
            condition: ModerationCondition::ContainsLinksInList {
                domains: vec!["spam.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsLinksOutsideList {
                domains: vec!["ok.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: ModerationCondition::ContainsLinksOutsideTop100 {
                domains: vec!["extra.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::IsBlank,
                    ModerationCondition::ContainsInvisibleCharacters,
                    ModerationCondition::ExceedsMaxCharacters {
                        max_characters: 100,
                    },
                    ModerationCondition::ExceedsMaxWords { max_words: 20 },
                    ModerationCondition::ExceedsMaxLines {
                        max_lines: 5,
                        chars_per_line: 40,
                    },
                ],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 5,
                time_window_minutes: 2,
            },
        },
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    // Verify data exists before deletion
    {
        let guard = conn.lock().unwrap();
        let action_count: i64 = guard
            .query_row("SELECT COUNT(*) FROM moderation_actions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(action_count, 8);
    }

    // Delete group data
    repo.delete_group_data(&messenger_group_id).await.unwrap();

    // Deleting the group must leave nothing behind anywhere. The table list
    // comes from the schema rather than being written out here, so a future
    // table hung off something other than the group fails this test instead of
    // being silently missed.
    {
        let guard = conn.lock().unwrap();
        let tables: Vec<String> = {
            let mut stmt = guard
                .prepare(
                    "SELECT name FROM sqlite_master
                      WHERE type = 'table' AND name LIKE 'moderation%'
                      ORDER BY name",
                )
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert!(
            tables.len() > 10,
            "expected the moderation schema to be discovered, found {tables:?}"
        );
        for table in tables {
            let count: i64 = guard
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                count, 0,
                "Table {table} should be empty after delete_group_data"
            );
        }
    }
}

#[tokio::test]
async fn test_save_and_load_rate_limit_rules() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let messenger_group_id = 1001;
    let owner_id = 456;
    let group_id = repo
        .save_owner(&messenger_group_id, "Rate Limit Test Group", &owner_id)
        .await
        .unwrap();

    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 1,
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 10,
                time_window_minutes: 60,
            },
        },
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].rule, rules[0]);
    assert_eq!(loaded[1].rule, rules[1]);
}

// ---------------------------------------------------------------------------
// Persistence round-trips
//
// Condition limits and normalization are the domain's job (see
// `message_filter::rule_condition`); what matters here is that a condition survives
// a save/load cycle unchanged, including its child-table entries.
// ---------------------------------------------------------------------------

async fn repo_with_group(messenger_group_id: i64) -> (SqliteModerationRepository, i64) {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();
    let repo = SqliteModerationRepository::new(conn);
    let group_id = repo
        .save_owner(&messenger_group_id, "Round Trip Test Group", &42)
        .await
        .unwrap();
    (repo, group_id)
}

async fn assert_round_trips(messenger_group_id: i64, condition: ModerationCondition) {
    let (repo, group_id) = repo_with_group(messenger_group_id).await;
    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition,
    }];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].rule, rules[0]);
}

#[tokio::test]
async fn test_round_trips_condition_with_a_child_table() {
    assert_round_trips(
        2001,
        ModerationCondition::ContainsWords {
            keywords: vec!["alpha".to_string(), "beta".to_string()],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_condition_with_settings_columns() {
    assert_round_trips(
        2002,
        ModerationCondition::ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 35,
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_every_message_shape_condition() {
    // Two of them have no parameters and so no table of their own: the type
    // tag in the registry has to be enough to bring them back.
    assert_round_trips(
        2008,
        ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::IsBlank,
                ModerationCondition::ContainsInvisibleCharacters,
                ModerationCondition::ExceedsMaxCharacters {
                    max_characters: 100,
                },
                ModerationCondition::ExceedsMaxWords { max_words: 20 },
                ModerationCondition::ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 0,
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_every_link_condition() {
    assert_round_trips(
        2009,
        ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::ContainsLinksInList {
                    domains: vec!["spam.com".to_string()],
                },
                ModerationCondition::ContainsLinksOutsideList {
                    domains: vec!["ok.com".to_string()],
                },
                ModerationCondition::ContainsLinksOutsideTop100 {
                    domains: vec!["extra.com".to_string()],
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_author_activity_conditions() {
    assert_round_trips(
        2010,
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 2,
                },
                ModerationCondition::AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_regex_patterns() {
    assert_round_trips(
        2003,
        ModerationCondition::MatchesRegex {
            patterns: vec![r" {5,}".to_string(), r"\d{3}-\d{4}".to_string()],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_repeated_sequence_settings() {
    assert_round_trips(
        2006,
        ModerationCondition::ContainsRepeatedSequence {
            min_repeats: 5,
            min_length: 2,
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_author_joined_recently_settings() {
    assert_round_trips(
        2007,
        ModerationCondition::AuthorJoinedRecently {
            time_window_minutes: 45,
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_a_nested_condition_tree() {
    assert_round_trips(
        2004,
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::Any {
                    conditions: vec![
                        ModerationCondition::ContainsWords {
                            keywords: vec!["alpha".to_string()],
                        },
                        ModerationCondition::MatchesRegex {
                            patterns: vec![r"\d{3}".to_string()],
                        },
                    ],
                },
                ModerationCondition::Not {
                    condition: Box::new(ModerationCondition::ContainsLinksOutsideList {
                        domains: vec!["ok.com".to_string()],
                    }),
                },
                ModerationCondition::AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 2,
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_preserves_sibling_order_within_a_composite() {
    // Children of a composite are ordered by `rank`, not by id or type, so a
    // composite whose children would sort differently under any other key still
    // comes back in the order it was written.
    assert_round_trips(
        2005,
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::MatchesRegex {
                    patterns: vec!["z".to_string()],
                },
                ModerationCondition::ContainsWords {
                    keywords: vec!["a".to_string()],
                },
                ModerationCondition::MatchesExactMessage {
                    messages: vec!["m".to_string()],
                    case_sensitive: false,
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_preserves_rule_order() {
    let (repo, group_id) = repo_with_group(2006).await;
    let rules: Vec<ModerationRule> = ["first", "second", "third", "fourth"]
        .iter()
        .map(|keyword| ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec![(*keyword).to_string()],
            },
        })
        .collect();

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    let loaded: Vec<ModerationRule> = loaded.into_iter().map(|owned| owned.rule).collect();
    assert_eq!(loaded, rules);
}

#[tokio::test]
async fn test_replacing_rules_leaves_no_orphan_actions() {
    let (repo, group_id) = repo_with_group(2007).await;
    let first = vec![ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        },
        condition: ModerationCondition::ContainsWords {
            keywords: vec!["alpha".to_string()],
        },
    }];
    repo.set_group_rules(&group_id, &first).await.unwrap();
    repo.set_group_rules(&group_id, &first).await.unwrap();
    repo.set_group_rules(&group_id, &first).await.unwrap();

    let guard = repo.conn.lock().unwrap();
    for table in [
        "moderation_actions",
        "moderation_action__kick_author",
        "moderation_rules",
    ] {
        let count: i64 = guard
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "{table} should hold exactly one row");
    }
}
