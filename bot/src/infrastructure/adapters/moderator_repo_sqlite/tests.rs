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
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["word1".to_string()],
            },
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::MatchesExactMessage {
                messages: vec!["msg1".to_string()],
                case_sensitive: true,
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::MatchesRegex {
                patterns: vec![r" {5,}".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::SetAuthorObserver {
                duration_minutes: 0,
            }],
            condition: ModerationCondition::ContainsLinksInList {
                domains: vec!["spam.com".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsLinksOutsideList {
                domains: vec!["ok.com".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: true,
            }],
            condition: ModerationCondition::ContainsLinksOutsideTop100 {
                domains: vec!["extra.com".to_string()],
            },
        },
        ModerationRule {
            actions: vec![
                ModerationAction::SetAuthorObserver {
                    duration_minutes: 0,
                },
                ModerationAction::ModerateMessage,
            ],
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
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: true,
            }],
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
        // Eight rules, two of which carry two actions each.
        assert_eq!(action_count, 10);
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
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 1,
            },
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
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

/// The line rate limit carries a third setting, the wrap width, which must come
/// back with the rest of it — including from inside a composite.
#[tokio::test]
async fn test_round_trips_line_rate_limit_settings() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let group_id = repo
        .save_owner(&1002, "Line Rate Limit Group", &456)
        .await
        .unwrap();

    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::All {
            conditions: vec![
                ModerationCondition::AuthorHitsLineRateLimit {
                    line_count: 30,
                    time_window_minutes: 5,
                    chars_per_line: 40,
                },
                ModerationCondition::Not {
                    condition: Box::new(ModerationCondition::AuthorHitsLineRateLimit {
                        line_count: 100,
                        time_window_minutes: 60,
                        chars_per_line: 0,
                    }),
                },
            ],
        },
    }];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].rule, rules[0]);
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
        actions: vec![ModerationAction::ModerateMessage],
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
async fn test_round_trips_every_attachment_condition() {
    // None of them has parameters, so the type tag in the registry is all
    // there is to tell the four apart.
    assert_round_trips(
        2012,
        ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::ContainsImage,
                ModerationCondition::ContainsVideo,
                ModerationCondition::ContainsVoiceMessage,
                ModerationCondition::ContainsFile,
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
                ModerationCondition::AuthorHitsCharacterRateLimit {
                    character_count: 2000,
                    time_window_minutes: 5,
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

/// The two rate limits are different conditions with different tables: a rule
/// using both must come back with both, not with one read into the other.
#[tokio::test]
async fn test_round_trips_both_rate_limits_side_by_side() {
    assert_round_trips(
        2011,
        ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::AuthorHitsMessageRateLimit {
                    message_count: 10,
                    time_window_minutes: 1,
                },
                ModerationCondition::AuthorHitsCharacterRateLimit {
                    character_count: 10,
                    time_window_minutes: 1,
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
            actions: vec![ModerationAction::ModerateMessage],
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
async fn test_preserves_action_order_within_a_rule() {
    let (repo, group_id) = repo_with_group(2008).await;
    let rules = vec![ModerationRule {
        actions: vec![
            ModerationAction::SetAuthorObserver {
                duration_minutes: 0,
            },
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ],
        condition: ModerationCondition::ContainsWords {
            keywords: vec!["alpha".to_string()],
        },
    }];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    // The rank column, not the insertion order of the rows, is what restores
    // the order the actions are executed in.
    assert_eq!(loaded[0].rule, rules[0]);
}

#[tokio::test]
async fn test_replacing_rules_leaves_no_orphan_actions() {
    let (repo, group_id) = repo_with_group(2007).await;
    let first = vec![ModerationRule {
        actions: vec![ModerationAction::KickAuthor {
            delete_all_messages: true,
        }],
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

#[tokio::test]
async fn test_round_trips_observer_duration() {
    let (repo, group_id) = repo_with_group(2008).await;
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::SetAuthorObserver {
                duration_minutes: 90,
            }],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["alpha".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::SetAuthorObserver {
                duration_minutes: 0,
            }],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["beta".to_string()],
            },
        },
    ];
    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    let actions: Vec<Vec<ModerationAction>> = loaded.into_iter().map(|r| r.rule.actions).collect();
    assert_eq!(
        actions,
        vec![
            vec![ModerationAction::SetAuthorObserver {
                duration_minutes: 90
            }],
            vec![ModerationAction::SetAuthorObserver {
                duration_minutes: 0
            }],
        ]
    );
}

// ---------------------------------------------------------------------------
// FlaggedByOmniModeration
// ---------------------------------------------------------------------------

use crate::domain::moderator::ports::{CategoryTrigger, OpenAiCategoryTriggers};

fn openai_condition(api_key: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOmniModeration {
        api_key: api_key.to_string(),
        triggers: OpenAiCategoryTriggers {
            hate: CategoryTrigger::OpenAiDecides,
            violence: CategoryTrigger::MinScorePercent(80),
            self_harm_intent: CategoryTrigger::MinScorePercent(1),
            sexual_minors: CategoryTrigger::MinScorePercent(100),
            ..Default::default()
        },
    }
}

async fn openai_repo(
    messenger_group_id: i64,
) -> (Arc<Mutex<Connection>>, SqliteModerationRepository, i64) {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();
    let repo = SqliteModerationRepository::new(conn.clone());
    let group_id = repo
        .save_owner(&messenger_group_id, "OpenAI Group", &42)
        .await
        .unwrap();
    (conn, repo, group_id)
}

fn count(conn: &Arc<Mutex<Connection>>, table: &str) -> i64 {
    conn.lock()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[tokio::test]
async fn test_round_trips_openai_moderation_condition() {
    assert_round_trips(2101, openai_condition("sk-proj-abc")).await;
}

#[tokio::test]
async fn test_round_trips_openai_moderation_condition_with_every_category_on() {
    assert_round_trips(
        2102,
        ModerationCondition::FlaggedByOmniModeration {
            api_key: "sk-proj-abc".to_string(),
            triggers: OpenAiCategoryTriggers::all(CategoryTrigger::OpenAiDecides),
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_two_openai_conditions_with_different_keys_in_one_tree() {
    assert_round_trips(
        2103,
        ModerationCondition::Any {
            conditions: vec![
                openai_condition("sk-one"),
                ModerationCondition::Not {
                    condition: Box::new(openai_condition("sk-two")),
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_openai_categories_store_one_row_per_category_that_is_not_off() {
    let (conn, repo, group_id) = openai_repo(2104).await;
    repo.set_group_rules(
        &group_id,
        &[ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openai_condition("sk-proj-abc"),
        }],
    )
    .await
    .unwrap();

    let guard = conn.lock().unwrap();
    let api_key: String = guard
        .query_row(
            "SELECT api_key FROM moderation_condition__flagged_by_omni_moderation",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(api_key, "sk-proj-abc");

    let mut stmt = guard
        .prepare(
            "SELECT category, min_score_percent
               FROM moderation_condition__flagged_by_omni_moderation__categories
              ORDER BY category",
        )
        .unwrap();
    let rows: Vec<(String, Option<i64>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // NULL is "OpenAI decides"; a category that is off has no row at all.
    assert_eq!(
        rows,
        vec![
            ("hate".to_string(), None),
            ("self_harm_intent".to_string(), Some(1)),
            ("sexual_minors".to_string(), Some(100)),
            ("violence".to_string(), Some(80)),
        ]
    );
}

#[tokio::test]
async fn test_openai_rows_go_with_replaced_rules_and_with_the_group() {
    let (conn, repo, group_id) = openai_repo(2105).await;
    let with_openai = [ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: openai_condition("sk-proj-abc"),
    }];

    repo.set_group_rules(&group_id, &with_openai).await.unwrap();
    repo.set_group_rules(&group_id, &with_openai).await.unwrap();
    assert_eq!(
        count(&conn, "moderation_condition__flagged_by_omni_moderation"),
        1
    );
    assert_eq!(
        count(
            &conn,
            "moderation_condition__flagged_by_omni_moderation__categories"
        ),
        4
    );

    repo.delete_group_data(&2105).await.unwrap();
    assert_eq!(
        count(&conn, "moderation_condition__flagged_by_omni_moderation"),
        0
    );
    assert_eq!(
        count(
            &conn,
            "moderation_condition__flagged_by_omni_moderation__categories"
        ),
        0
    );
}

// ---------------------------------------------------------------------------
// FlaggedByOpenAiInstruction
// ---------------------------------------------------------------------------

fn instructed(api_key: &str, model: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOpenAiInstruction {
        api_key: api_key.to_string(),
        model: model.to_string(),
        instruction: "Block crypto ads.\nAllow \"quotes\" and ünïcode — всё.".to_string(),
    }
}

#[tokio::test]
async fn test_round_trips_openai_instruction_condition() {
    assert_round_trips(2201, instructed("sk-proj-abc", "gpt-4o-mini")).await;
}

#[tokio::test]
async fn test_round_trips_openai_instruction_beside_openai_moderation_in_one_tree() {
    assert_round_trips(
        2202,
        ModerationCondition::All {
            conditions: vec![
                openai_condition("sk-one"),
                ModerationCondition::Not {
                    condition: Box::new(instructed("sk-two", "gpt-4.1-mini")),
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_openai_instruction_rows_go_with_replaced_rules_and_with_the_group() {
    let (conn, repo, group_id) = openai_repo(2203).await;
    let rules = [ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: instructed("sk-proj-abc", "gpt-4o-mini"),
    }];

    repo.set_group_rules(&group_id, &rules).await.unwrap();
    repo.set_group_rules(&group_id, &rules).await.unwrap();
    assert_eq!(
        count(&conn, "moderation_condition__flagged_by_openai_instruction"),
        1
    );

    repo.delete_group_data(&2203).await.unwrap();
    assert_eq!(
        count(&conn, "moderation_condition__flagged_by_openai_instruction"),
        0
    );
}
