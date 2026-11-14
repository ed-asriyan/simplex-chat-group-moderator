use super::*;
use crate::domain::moderator::ports::ModerationRule;
use crate::domain::moderator::ports::RuleDraft;
use crate::domain::moderator::ports::actions::{KickAuthor, ModerateMessage, SetAuthorObserver};
use crate::domain::moderator::ports::conditions::{
    AuthorHitsCharacterRateLimit, AuthorHitsLineRateLimit, AuthorHitsMessageRateLimit,
    AuthorHitsModerationRateLimit, AuthorJoinedRecently, ContainsFile, ContainsImage,
    ContainsInvisibleCharacters, ContainsLinksInList, ContainsLinksOutsideList,
    ContainsLinksOutsideTop100, ContainsRepeatedSequence, ContainsVideo, ContainsVoiceMessage,
    ContainsWords, ExceedsMaxCharacters, ExceedsMaxLines, ExceedsMaxWords, FlaggedByOmniModeration,
    FlaggedByOpenRouterInstruction, GroupHitsCharacterRateLimit, GroupHitsLineRateLimit,
    GroupHitsMessageRateLimit, IsBlank, MatchesExactMessage, MatchesRegex,
};
use crate::domain::moderator::rules::test_support::message_rule;
use crate::infrastructure::migrations;

/// Most tests of the repository, here and in the migrations, are about the
/// rules alone, so they save a configuration of nothing else.
impl SqliteModerationRepository {
    pub(crate) async fn set_group_rules(
        &self,
        group_id: &GroupId,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let config = GroupConfig {
            rules: rules.to_vec(),
        };
        self.set_group_config(group_id, &config).await
    }

    pub(crate) async fn get_group_rules(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<ModerationRule>, Err> {
        Ok(self.get_group_config(group_id).await?.rules)
    }
}

// US-B6: forgetting a group removes its rules, with their conditions, actions and
// both switches, so the group is no longer in the owner's groups.
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
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["word1".to_string()],
            }),
        ),
        message_rule(
            vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            ModerationCondition::MatchesExactMessage(MatchesExactMessage {
                messages: vec!["msg1".to_string()],
                case_sensitive: true,
            }),
        ),
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::MatchesRegex(MatchesRegex {
                patterns: vec![r" {5,}".to_string()],
            }),
        ),
        message_rule(
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0,
            })],
            ModerationCondition::ContainsLinksInList(ContainsLinksInList {
                domains: vec!["spam.com".to_string()],
            }),
        ),
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::ContainsLinksOutsideList(ContainsLinksOutsideList {
                domains: vec!["ok.com".to_string()],
            }),
        ),
        message_rule(
            vec![ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: true,
            })],
            ModerationCondition::ContainsLinksOutsideTop100(ContainsLinksOutsideTop100 {
                domains: vec!["extra.com".to_string()],
            }),
        ),
        message_rule(
            vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::IsBlank(IsBlank {}),
                    ModerationCondition::ContainsInvisibleCharacters(
                        ContainsInvisibleCharacters {},
                    ),
                    ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters {
                        max_characters: 100,
                    }),
                    ModerationCondition::ExceedsMaxWords(ExceedsMaxWords { max_words: 20 }),
                    ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
                        max_lines: 5,
                        chars_per_line: 40,
                    }),
                ],
            },
        ),
        message_rule(
            vec![ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: true,
            })],
            ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                message_count: 5,
                time_window_minutes: 2,
            }),
        ),
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
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 1,
            }),
        ),
        message_rule(
            vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                message_count: 10,
                time_window_minutes: 60,
            }),
        ),
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0], rules[0]);
    assert_eq!(loaded[1], rules[1]);
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

    let rules = vec![message_rule(
        vec![ModerationAction::ModerateMessage(ModerateMessage {})],
        ModerationCondition::All {
            conditions: vec![
                ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                    line_count: 30,
                    time_window_minutes: 5,
                    chars_per_line: 40,
                }),
                ModerationCondition::Not {
                    condition: Box::new(ModerationCondition::AuthorHitsLineRateLimit(
                        AuthorHitsLineRateLimit {
                            line_count: 100,
                            time_window_minutes: 60,
                            chars_per_line: 0,
                        },
                    )),
                },
            ],
        },
    )];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0], rules[0]);
}

/// The group-wide rate limits have tables of their own, apart from the
/// author's; each has to come back with its settings, nested or not.
#[tokio::test]
async fn test_round_trips_group_rate_limit_settings() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let group_id = repo
        .save_owner(&1003, "Group Rate Limit Group", &456)
        .await
        .unwrap();

    let rules = vec![
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::GroupHitsMessageRateLimit(GroupHitsMessageRateLimit {
                message_count: 50,
                time_window_minutes: 1,
            }),
        ),
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::Any {
                conditions: vec![
                    ModerationCondition::GroupHitsCharacterRateLimit(GroupHitsCharacterRateLimit {
                        character_count: 10000,
                        time_window_minutes: 5,
                    }),
                    ModerationCondition::GroupHitsLineRateLimit(GroupHitsLineRateLimit {
                        line_count: 200,
                        time_window_minutes: 10,
                        chars_per_line: 40,
                    }),
                ],
            },
        ),
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0], rules[0]);
    assert_eq!(loaded[1], rules[1]);
}

// ---------------------------------------------------------------------------
// Persistence round-trips
//
// Condition limits and normalization are the domain's job (see
// `rules::moderation_condition`); what matters here is that a condition survives
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
    let rules = vec![message_rule(
        vec![ModerationAction::ModerateMessage(ModerateMessage {})],
        condition,
    )];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0], rules[0]);
}

#[tokio::test]
async fn test_round_trips_condition_with_a_child_table() {
    assert_round_trips(
        2001,
        ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["alpha".to_string(), "beta".to_string()],
        }),
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_condition_with_settings_columns() {
    assert_round_trips(
        2002,
        ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
            max_lines: 5,
            chars_per_line: 35,
        }),
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
                ModerationCondition::IsBlank(IsBlank {}),
                ModerationCondition::ContainsInvisibleCharacters(ContainsInvisibleCharacters {}),
                ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters {
                    max_characters: 100,
                }),
                ModerationCondition::ExceedsMaxWords(ExceedsMaxWords { max_words: 20 }),
                ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 0,
                }),
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
                ModerationCondition::ContainsImage(ContainsImage {}),
                ModerationCondition::ContainsVideo(ContainsVideo {}),
                ModerationCondition::ContainsVoiceMessage(ContainsVoiceMessage {}),
                ModerationCondition::ContainsFile(ContainsFile {}),
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
                ModerationCondition::ContainsLinksInList(ContainsLinksInList {
                    domains: vec!["spam.com".to_string()],
                }),
                ModerationCondition::ContainsLinksOutsideList(ContainsLinksOutsideList {
                    domains: vec!["ok.com".to_string()],
                }),
                ModerationCondition::ContainsLinksOutsideTop100(ContainsLinksOutsideTop100 {
                    domains: vec!["extra.com".to_string()],
                }),
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
                ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 2,
                }),
                ModerationCondition::AuthorHitsCharacterRateLimit(AuthorHitsCharacterRateLimit {
                    character_count: 2000,
                    time_window_minutes: 5,
                }),
                ModerationCondition::AuthorHitsModerationRateLimit(AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                }),
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
                ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                    message_count: 10,
                    time_window_minutes: 1,
                }),
                ModerationCondition::AuthorHitsCharacterRateLimit(AuthorHitsCharacterRateLimit {
                    character_count: 10,
                    time_window_minutes: 1,
                }),
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_regex_patterns() {
    assert_round_trips(
        2003,
        ModerationCondition::MatchesRegex(MatchesRegex {
            patterns: vec![r" {5,}".to_string(), r"\d{3}-\d{4}".to_string()],
        }),
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_repeated_sequence_settings() {
    assert_round_trips(
        2006,
        ModerationCondition::ContainsRepeatedSequence(ContainsRepeatedSequence {
            min_repeats: 5,
            min_length: 2,
        }),
    )
    .await;
}

#[tokio::test]
async fn test_round_trips_author_joined_recently_settings() {
    assert_round_trips(
        2007,
        ModerationCondition::AuthorJoinedRecently(AuthorJoinedRecently {
            time_window_minutes: 45,
        }),
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
                        ModerationCondition::ContainsWords(ContainsWords {
                            keywords: vec!["alpha".to_string()],
                        }),
                        ModerationCondition::MatchesRegex(MatchesRegex {
                            patterns: vec![r"\d{3}".to_string()],
                        }),
                    ],
                },
                ModerationCondition::Not {
                    condition: Box::new(ModerationCondition::ContainsLinksOutsideList(
                        ContainsLinksOutsideList {
                            domains: vec!["ok.com".to_string()],
                        },
                    )),
                },
                ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 2,
                }),
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
                ModerationCondition::MatchesRegex(MatchesRegex {
                    patterns: vec!["z".to_string()],
                }),
                ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["a".to_string()],
                }),
                ModerationCondition::MatchesExactMessage(MatchesExactMessage {
                    messages: vec!["m".to_string()],
                    case_sensitive: false,
                }),
            ],
        },
    )
    .await;
}

// US-C1, US-B4, US-G13, US-G19: a rule is stored with its two switches, every
// combination of them comes back as it went in, and a rule with none turned off
// is stored as one that is on and tells.
#[tokio::test]
async fn test_round_trips_each_rules_two_switches() {
    use crate::domain::moderator::rules::test_support::switched;
    let (repo, group_id) = repo_with_group(2010).await;
    let rule = || {
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::IsBlank(IsBlank {}),
        )
    };

    // A new group has no rules.
    assert!(repo.get_group_rules(&group_id).await.unwrap().is_empty());

    let rules = vec![
        switched(rule(), true, true),
        switched(rule(), false, true),
        switched(rule(), true, false),
        switched(rule(), false, false),
    ];
    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded, rules);
    let flags: Vec<(bool, bool)> = loaded.iter().map(|r| (r.enabled, r.notify)).collect();
    assert_eq!(
        flags,
        vec![(true, true), (false, true), (true, false), (false, false)]
    );
    // The group is there, with no mode of its own to read.
    let group = repo
        .get_group_by_messenger_id(&2010)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.id, group_id);
}

#[tokio::test]
async fn test_preserves_rule_order() {
    let (repo, group_id) = repo_with_group(2006).await;
    let rules: Vec<ModerationRule> = ["first", "second", "third", "fourth"]
        .iter()
        .map(|keyword| {
            message_rule(
                vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec![(*keyword).to_string()],
                }),
            )
        })
        .collect();

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded, rules);
}

#[tokio::test]
async fn test_preserves_action_order_within_a_rule() {
    let (repo, group_id) = repo_with_group(2008).await;
    let rules = vec![message_rule(
        vec![
            ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0,
            }),
            ModerationAction::ModerateMessage(ModerateMessage {}),
            ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: false,
            }),
        ],
        ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["alpha".to_string()],
        }),
    )];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 1);
    // The rank column, not the insertion order of the rows, is what restores
    // the order the actions are executed in.
    assert_eq!(loaded[0], rules[0]);
}

#[tokio::test]
async fn test_replacing_rules_leaves_no_orphan_actions() {
    let (repo, group_id) = repo_with_group(2007).await;
    let first = vec![message_rule(
        vec![ModerationAction::KickAuthor(KickAuthor {
            delete_all_messages: true,
        })],
        ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["alpha".to_string()],
        }),
    )];
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
        message_rule(
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 90,
            })],
            ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["alpha".to_string()],
            }),
        ),
        message_rule(
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0,
            })],
            ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["beta".to_string()],
            }),
        ),
    ];
    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    let actions: Vec<Vec<ModerationAction>> = loaded
        .into_iter()
        .map(|r| RuleDraft::from(r).actions)
        .collect();
    assert_eq!(
        actions,
        vec![
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 90
            })],
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0
            })],
        ]
    );
}

// ---------------------------------------------------------------------------
// FlaggedByOmniModeration
// ---------------------------------------------------------------------------

use crate::domain::moderator::ports::{CategoryTrigger, OpenAiCategoryTriggers};

fn openai_condition(api_key: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOmniModeration(FlaggedByOmniModeration {
        retry: Default::default(),
        api_key: api_key.to_string(),
        triggers: OpenAiCategoryTriggers {
            hate: CategoryTrigger::OpenAiDecides,
            violence: CategoryTrigger::MinScorePercent(80),
            self_harm_intent: CategoryTrigger::MinScorePercent(1),
            sexual_minors: CategoryTrigger::MinScorePercent(100),
            ..Default::default()
        },
    })
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
        ModerationCondition::FlaggedByOmniModeration(FlaggedByOmniModeration {
            retry: Default::default(),
            api_key: "sk-proj-abc".to_string(),
            triggers: OpenAiCategoryTriggers::all(CategoryTrigger::OpenAiDecides),
        }),
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
        &[message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            openai_condition("sk-proj-abc"),
        )],
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
    let with_openai = [message_rule(
        vec![ModerationAction::ModerateMessage(ModerateMessage {})],
        openai_condition("sk-proj-abc"),
    )];

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
// FlaggedByOpenRouterInstruction
// ---------------------------------------------------------------------------

fn instructed(api_key: &str, model: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOpenRouterInstruction(FlaggedByOpenRouterInstruction {
        retry: Default::default(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        instruction: "Block crypto ads.\nAllow \"quotes\" and ünïcode — всё.".to_string(),
        context_messages: 3,
    })
}

#[tokio::test]
async fn test_round_trips_openrouter_instruction_condition() {
    assert_round_trips(2201, instructed("sk-or-v1-abc", "openai/gpt-4o-mini")).await;
}

#[tokio::test]
async fn test_round_trips_openrouter_instruction_beside_omni_moderation_in_one_tree() {
    assert_round_trips(
        2202,
        ModerationCondition::All {
            conditions: vec![
                openai_condition("sk-one"),
                ModerationCondition::Not {
                    condition: Box::new(instructed("sk-or-two", "google/gemini-2.5-flash-lite")),
                },
            ],
        },
    )
    .await;
}

#[tokio::test]
async fn test_openrouter_instruction_rows_go_with_replaced_rules_and_with_the_group() {
    let (conn, repo, group_id) = openai_repo(2203).await;
    let rules = [message_rule(
        vec![ModerationAction::ModerateMessage(ModerateMessage {})],
        instructed("sk-or-v1-abc", "openai/gpt-4o-mini"),
    )];

    repo.set_group_rules(&group_id, &rules).await.unwrap();
    repo.set_group_rules(&group_id, &rules).await.unwrap();
    assert_eq!(
        count(
            &conn,
            "moderation_condition__flagged_by_openrouter_instruction"
        ),
        1
    );

    repo.delete_group_data(&2203).await.unwrap();
    assert_eq!(
        count(
            &conn,
            "moderation_condition__flagged_by_openrouter_instruction"
        ),
        0
    );
}

// ---------------------------------------------------------------------------
// Triggers, names and switches; the conditions on a member's name
// ---------------------------------------------------------------------------

use crate::domain::moderator::ports::Trigger;
use crate::domain::moderator::ports::conditions::{
    MemberNameContainsLinksOutsideList, MemberNameContainsWords,
};
use crate::domain::moderator::rules::test_support::{join_rule, named};

fn kick() -> ModerationAction {
    ModerationAction::KickAuthor(KickAuthor {
        delete_all_messages: false,
    })
}

fn casino() -> ModerationCondition {
    ModerationCondition::MemberNameContainsWords(MemberNameContainsWords {
        keywords: vec!["casino".to_string(), "bonus".to_string()],
    })
}

// US-E31
#[tokio::test]
async fn test_round_trips_the_condition_on_words_in_a_name() {
    assert_round_trips(3001, casino()).await;
}

// US-E32
#[tokio::test]
async fn test_round_trips_the_condition_on_links_in_a_name() {
    assert_round_trips(
        3002,
        ModerationCondition::MemberNameContainsLinksOutsideList(
            MemberNameContainsLinksOutsideList {
                domains: vec!["simplex.chat".to_string(), "example.org".to_string()],
            },
        ),
    )
    .await;
    // An empty list is a rule too: any link matches.
    assert_round_trips(
        3003,
        ModerationCondition::MemberNameContainsLinksOutsideList(
            MemberNameContainsLinksOutsideList { domains: vec![] },
        ),
    )
    .await;
}

// US-C1, US-G13: a rule's name, its trigger and its switch are stored with it.
#[tokio::test]
async fn test_round_trips_a_rules_name_trigger_and_switch() {
    let (repo, group_id) = repo_with_group(3004).await;
    let rules = vec![
        named(
            message_rule(
                vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["spam".to_string()],
                }),
            ),
            "Banned words",
            true,
        ),
        named(join_rule(vec![kick()], Some(casino())), "Spam names", false),
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::IsBlank(IsBlank {}),
        ),
    ];
    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded, rules);
    assert_eq!(loaded[0].name.as_deref(), Some("Banned words"));
    assert_eq!(loaded[1].trigger(), Trigger::MemberJoined);
    assert!(!loaded[1].enabled);
    assert_eq!(loaded[2].name, None);
    assert!(loaded[2].enabled);
}

// US-D6: a rule for joins may have no condition, which stores no condition row.
#[tokio::test]
async fn test_round_trips_a_join_rule_with_no_condition() {
    let (repo, group_id) = repo_with_group(3005).await;
    let rules = vec![named(
        join_rule(
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 60,
            })],
            None,
        ),
        "Quarantine newcomers",
        true,
    )];
    repo.set_group_rules(&group_id, &rules).await.unwrap();
    assert_eq!(repo.get_group_rules(&group_id).await.unwrap(), rules);
    let conn = repo.conn.lock().unwrap();
    let conditions: i64 = conn
        .query_row("SELECT COUNT(*) FROM moderation_conditions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(conditions, 0);
}

// US-D1, US-D6: one order across both triggers, each trigger's rules keeping theirs.
#[tokio::test]
async fn test_preserves_the_order_of_rules_of_both_triggers() {
    let (repo, group_id) = repo_with_group(3006).await;
    let message = |keyword: &str| {
        message_rule(
            vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec![keyword.to_string()],
            }),
        )
    };
    let rules = vec![
        message("a"),
        join_rule(vec![kick()], None),
        message("b"),
        named(join_rule(vec![kick()], Some(casino())), "Names", true),
    ];
    repo.set_group_rules(&group_id, &rules).await.unwrap();
    assert_eq!(repo.get_group_rules(&group_id).await.unwrap(), rules);
}

// US-B4, US-G13: the rules a group has are all handed back, those turned off
// included — the bot skips them when it checks, it does not forget them.
#[tokio::test]
async fn test_a_rule_that_is_off_is_still_in_the_configuration_and_in_the_rules() {
    let (repo, group_id) = repo_with_group(3007).await;
    let off = named(join_rule(vec![kick()], None), "Everyone out", false);
    repo.set_group_rules(&group_id, &[off.clone()])
        .await
        .unwrap();
    assert_eq!(
        repo.get_group_config(&group_id).await.unwrap().rules,
        vec![off.clone()]
    );
    let owned = repo.get_group_rules_by_messenger_id(&3007).await.unwrap();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].rule, off);
}

// US-C1: replacing the configuration replaces the name-condition rows too.
#[tokio::test]
async fn test_replacing_rules_leaves_no_orphan_name_conditions() {
    let (repo, group_id) = repo_with_group(3008).await;
    repo.set_group_rules(
        &group_id,
        &[
            join_rule(vec![kick()], Some(casino())),
            join_rule(
                vec![kick()],
                Some(ModerationCondition::MemberNameContainsLinksOutsideList(
                    MemberNameContainsLinksOutsideList {
                        domains: vec!["a.com".to_string()],
                    },
                )),
            ),
        ],
    )
    .await
    .unwrap();
    repo.set_group_rules(&group_id, &[]).await.unwrap();
    for table in [
        "moderation_condition__member_name_contains_words__keywords",
        "moderation_condition__member_name_contains_links_outside_list__domains",
    ] {
        let conn = repo.conn.lock().unwrap();
        let rows: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0, "{table}");
    }
}
