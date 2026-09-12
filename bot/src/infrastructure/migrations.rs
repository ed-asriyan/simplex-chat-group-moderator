use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::domain::moderator::ports::Err;

use include_dir::{Dir, include_dir};

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/infrastructure/migrations");

/// Apply all pending migrations, bringing the database schema up to date.
///
/// This is idempotent: migrations already recorded in `PRAGMA user_version` are
/// skipped. Each migration runs inside its own transaction together with the
/// version bump, so a failure leaves the database at the last fully-applied
/// version.
pub async fn run(conn: Arc<Mutex<Connection>>) -> Result<(), Err> {
    tokio::task::spawn_blocking(move || -> Result<(), Err> {
        let mut guard = conn.lock().expect("migration connection poisoned");
        apply(&mut guard)
    })
    .await
    .map_err(|e| -> Err { e.to_string().into() })?
}

fn apply(conn: &mut Connection) -> Result<(), Err> {
    apply_through(conn, usize::MAX)
}

/// Apply pending migrations up to and including `target` (1-based position in
/// the sorted file list). Tests use this to stop at an older schema and check
/// that the next migration carries real data across.
fn apply_through(conn: &mut Connection, target: usize) -> Result<(), Err> {
    // SQLite disables foreign key enforcement per-connection by default. Enable
    // it here so ON DELETE CASCADE constraints (e.g. group -> keywords) are
    // honoured for both the migrations below and all later queries on this
    // connection. This pragma is a no-op inside a transaction, so it must be set
    // before the per-migration transactions begin.
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let current = current.max(0) as usize;

    // Get all files, filter for .sql, and sort them strictly by name (e.g. 0001_..., 0002_...)
    let mut files: Vec<_> = MIGRATIONS_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort_by_key(|f| f.path());

    for (idx, file) in files.iter().enumerate() {
        let version = idx + 1;
        if version <= current || version > target {
            continue;
        }

        let sql = file
            .contents_utf8()
            .ok_or("migration file is not valid UTF-8")?;

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // PRAGMA does not accept bound parameters; version is a trusted usize.
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_apply_successfully() {
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        run(conn.clone()).await.unwrap();

        let guard = conn.lock().unwrap();
        let version: i64 = guard
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 26);
    }

    /// 0022 rebuilds every rule as a `moderation_rules` row plus a condition
    /// tree, and flips the rule/action foreign key. Seed the schema it replaces
    /// with real rows and check they come out the other side intact, in order,
    /// still attached to their actions.
    #[tokio::test]
    async fn test_0022_migrates_existing_rules_into_condition_trees() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        apply_through(&mut conn, 21).unwrap();

        conn.execute_batch(
            "INSERT INTO moderation_groups (group_id, messenger_group_id, owner_id, group_name)
                  VALUES (7, 700, 70, 'Legacy Group');

             -- rank 0: banned words, kept on the default action (action_id NULL)
             INSERT INTO moderation_rule__contains_banned_words (id, group_id, rank, action_id)
                  VALUES (1, 7, 0, NULL);
             INSERT INTO moderation_rule__contains_banned_words__keywords (rule_id, keyword)
                  VALUES (1, 'alpha'), (1, 'beta');

             -- rank 1: exact message with a settings column and a KickAuthor action
             INSERT INTO moderation_actions (id, type) VALUES (500, 'KickAuthor');
             INSERT INTO moderation_action__kick_author (action_id, delete_messages)
                  VALUES (500, 2);
             INSERT INTO moderation_rule__matches_exact_message (id, group_id, rank, case_sensitive, action_id)
                  VALUES (1, 7, 1, 1, 500);
             INSERT INTO moderation_rule__matches_exact_message__messages (rule_id, message)
                  VALUES (1, 'spam');

             -- rank 2: rate limit, settings only
             INSERT INTO moderation_rule__user_exceeds_messages_rate_limit
                  (id, group_id, rank, message_count, time_window_minutes, action_id)
                  VALUES (1, 7, 2, 5, 3, NULL);

             -- An action that already lost its rule under the old schema.
             INSERT INTO moderation_actions (id, type) VALUES (900, 'ModerateMessage');
             INSERT INTO moderation_action__moderate_message (action_id) VALUES (900);",
        )
        .unwrap();

        // Through the latest migration, not just 0022: the repository below
        // reads the current schema, so every later table must exist too.
        apply(&mut conn).unwrap();

        let conn = Arc::new(Mutex::new(conn));
        let repo =
            crate::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository::new(
                conn.clone(),
            );
        let loaded = repo.get_group_rules(&7).await.unwrap();
        let loaded: Vec<_> = loaded.into_iter().map(|owned| owned.rule).collect();

        use crate::domain::moderator::ports::{
            ModerationAction, ModerationCondition, ModerationRepository, ModerationRule,
        };
        assert_eq!(
            loaded,
            vec![
                ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage],
                    condition: ModerationCondition::ContainsWords {
                        keywords: vec!["alpha".to_string(), "beta".to_string()],
                    },
                },
                ModerationRule {
                    actions: vec![ModerationAction::KickAuthor {
                        delete_all_messages: true,
                    }],
                    condition: ModerationCondition::MatchesExactMessage {
                        messages: vec!["spam".to_string()],
                        case_sensitive: true,
                    },
                },
                ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage],
                    condition: ModerationCondition::AuthorHitsMessageRateLimit {
                        message_count: 5,
                        time_window_minutes: 3,
                    },
                },
            ]
        );

        let guard = conn.lock().unwrap();
        // The pre-existing orphan is gone, and every surviving action now
        // belongs to a rule.
        let orphans: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM moderation_actions WHERE rule_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
        let settings: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM moderation_action__moderate_message",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            settings, 0,
            "the orphan action's settings row should be gone"
        );
    }

    /// 0025 renames most condition types and their tables, and splits
    /// `FloodsChatOrExceedsLimits` into one condition per check. Seed the
    /// schema it replaces with one rule per case and check every rule reads
    /// back as the condition it now is, in order, with its settings.
    #[tokio::test]
    async fn test_0025_renames_conditions_and_splits_floods() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        apply_through(&mut conn, 24).unwrap();

        conn.execute_batch(
            "INSERT INTO moderation_groups (group_id, messenger_group_id, owner_id, group_name)
                  VALUES (9, 900, 90, 'Renamed Group');

             INSERT INTO moderation_rules (id, group_id, rank) VALUES
                 (1, 9, 0), (2, 9, 1), (3, 9, 2), (4, 9, 3), (5, 9, 4), (6, 9, 5),
                 (7, 9, 6), (8, 9, 7), (9, 9, 8), (10, 9, 9), (11, 9, 10), (12, 9, 11);

             -- Plain renames, each with its settings or list rows.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type) VALUES
                 (1, 1, NULL, 0, 'ContainsBannedWords'),
                 (2, 2, NULL, 0, 'ContainsLinksToForbiddenWebsites'),
                 (3, 3, NULL, 0, 'ContainsLinksOutsideAllowedList'),
                 (4, 4, NULL, 0, 'ContainsLinksOutsideTop100'),
                 (5, 5, NULL, 0, 'UserExceedsMessagesRateLimit'),
                 (6, 6, NULL, 0, 'UserExceedsModerationRateLimit'),
                 (7, 7, NULL, 0, 'UserJoinedRecently');
             INSERT INTO moderation_condition__contains_banned_words__keywords
                  VALUES (1, 'alpha'), (1, 'beta');
             INSERT INTO moderation_condition__contains_links_to_forbidden_websites__domains
                  VALUES (2, 'spam.com');
             INSERT INTO moderation_condition__contains_links_outside_allowed_list__domains
                  VALUES (3, 'ok.com');
             INSERT INTO moderation_condition__contains_links_outside_top100__allowed
                  VALUES (4, 'extra.com');
             INSERT INTO moderation_condition__user_exceeds_messages_rate_limit VALUES (5, 5, 2);
             INSERT INTO moderation_condition__user_exceeds_moderation_rate_limit VALUES (6, 3, 60);
             INSERT INTO moderation_condition__user_joined_recently VALUES (7, 30);

             -- Floods with every check on: becomes an Any of all five, in order.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type)
                  VALUES (8, 8, NULL, 0, 'FloodsChatOrExceedsLimits');
             INSERT INTO moderation_condition__floods_chat_or_exceeds_limits
                  (condition_id, max_characters, max_words, max_lines, chars_per_line,
                   disallow_invisible_chars, disallow_empty_messages)
                  VALUES (8, 100, 20, 5, 35, 1, 1);

             -- Floods with a single check: replaced in place by that check,
             -- keeping its position inside the surrounding tree.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type) VALUES
                 (9, 9, NULL, 0, 'All'),
                 (10, 9, 9, 0, 'ContainsBannedWords'),
                 (11, 9, 9, 1, 'FloodsChatOrExceedsLimits');
             INSERT INTO moderation_condition__contains_banned_words__keywords VALUES (10, 'gamma');
             INSERT INTO moderation_condition__floods_chat_or_exceeds_limits
                  (condition_id, max_characters, max_words, max_lines, chars_per_line,
                   disallow_invisible_chars, disallow_empty_messages)
                  VALUES (11, 0, 0, 3, 0, 0, 0);

             -- Floods with two checks under a Not: the Not keeps a single
             -- child, which is now an Any.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type) VALUES
                 (12, 10, NULL, 0, 'Not'),
                 (13, 10, 12, 0, 'FloodsChatOrExceedsLimits');
             INSERT INTO moderation_condition__floods_chat_or_exceeds_limits
                  (condition_id, max_characters, max_words, max_lines, chars_per_line,
                   disallow_invisible_chars, disallow_empty_messages)
                  VALUES (13, 0, 7, 0, 40, 1, 0);

             -- Floods with every check off: never matched, and still doesn't.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type)
                  VALUES (14, 11, NULL, 0, 'FloodsChatOrExceedsLimits');
             INSERT INTO moderation_condition__floods_chat_or_exceeds_limits
                  (condition_id, max_characters, max_words, max_lines, chars_per_line,
                   disallow_invisible_chars, disallow_empty_messages)
                  VALUES (14, 0, 0, 0, 40, 0, 0);

             -- Floods without its settings row: the defaults the reader used to
             -- fall back to matched blank messages only.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type)
                  VALUES (15, 12, NULL, 0, 'FloodsChatOrExceedsLimits');",
        )
        .unwrap();

        apply(&mut conn).unwrap();

        use crate::domain::moderator::ports::{ModerationCondition as C, ModerationRepository};
        let conn = Arc::new(Mutex::new(conn));
        let repo =
            crate::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository::new(
                conn.clone(),
            );
        let loaded: Vec<C> = repo
            .get_group_rules(&9)
            .await
            .unwrap()
            .into_iter()
            .map(|owned| owned.rule.condition)
            .collect();

        let strings = |values: &[&str]| values.iter().map(|v| v.to_string()).collect();
        assert_eq!(
            loaded,
            vec![
                C::ContainsWords {
                    keywords: strings(&["alpha", "beta"]),
                },
                C::ContainsLinksInList {
                    domains: strings(&["spam.com"]),
                },
                C::ContainsLinksOutsideList {
                    domains: strings(&["ok.com"]),
                },
                C::ContainsLinksOutsideTop100 {
                    domains: strings(&["extra.com"]),
                },
                C::AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 2,
                },
                C::AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
                C::AuthorJoinedRecently {
                    time_window_minutes: 30,
                },
                C::Any {
                    conditions: vec![
                        C::IsBlank,
                        C::ContainsInvisibleCharacters,
                        C::ExceedsMaxCharacters {
                            max_characters: 100,
                        },
                        C::ExceedsMaxWords { max_words: 20 },
                        C::ExceedsMaxLines {
                            max_lines: 5,
                            chars_per_line: 35,
                        },
                    ],
                },
                C::All {
                    conditions: vec![
                        C::ContainsWords {
                            keywords: strings(&["gamma"]),
                        },
                        C::ExceedsMaxLines {
                            max_lines: 3,
                            chars_per_line: 0,
                        },
                    ],
                },
                C::Not {
                    condition: Box::new(C::Any {
                        conditions: vec![
                            C::ContainsInvisibleCharacters,
                            C::ExceedsMaxWords { max_words: 7 },
                        ],
                    }),
                },
                C::Any { conditions: vec![] },
                C::IsBlank,
            ]
        );

        let guard = conn.lock().unwrap();
        let leftovers: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                  WHERE type = 'table'
                    AND name IN (
                        'moderation_condition__contains_banned_words__keywords',
                        'moderation_condition__contains_links_to_forbidden_websites__domains',
                        'moderation_condition__contains_links_outside_allowed_list__domains',
                        'moderation_condition__contains_links_outside_top100__allowed',
                        'moderation_condition__floods_chat_or_exceeds_limits',
                        'moderation_condition__user_exceeds_messages_rate_limit',
                        'moderation_condition__user_exceeds_moderation_rate_limit',
                        'moderation_condition__user_joined_recently'
                    )",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(leftovers, 0, "every old table should be renamed or dropped");
        let temp_tables: i64 = guard
            .query_row("SELECT COUNT(*) FROM sqlite_temp_master", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            temp_tables, 0,
            "the migration's scratch tables should be gone"
        );
    }

    /// 0026 turns the single action per rule into a list and flattens the two
    /// actions that carried a "...and what about the message?" setting. Seed the
    /// schema it replaces with one rule per old case and check each reads back as
    /// the sequence of actions that means the same thing, in execution order.
    #[tokio::test]
    async fn test_0026_splits_tri_state_actions_into_action_lists() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        apply_through(&mut conn, 25).unwrap();

        conn.execute_batch(
            "INSERT INTO moderation_groups (group_id, messenger_group_id, owner_id, group_name)
                  VALUES (11, 1100, 110, 'Action Group');

             INSERT INTO moderation_rules (id, group_id, rank) VALUES
                 (1, 11, 0), (2, 11, 1), (3, 11, 2), (4, 11, 3), (5, 11, 4), (6, 11, 5),
                 (7, 11, 6);

             -- One condition per rule; the conditions are irrelevant here, they
             -- only make the rules loadable.
             INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type) VALUES
                 (1, 1, NULL, 0, 'ContainsWords'),
                 (2, 2, NULL, 0, 'ContainsWords'),
                 (3, 3, NULL, 0, 'ContainsWords'),
                 (4, 4, NULL, 0, 'ContainsWords'),
                 (5, 5, NULL, 0, 'ContainsWords'),
                 (6, 6, NULL, 0, 'ContainsWords'),
                 (7, 7, NULL, 0, 'ContainsWords');
             INSERT INTO moderation_condition__contains_words__keywords VALUES
                 (1, 'one'), (2, 'two'), (3, 'three'), (4, 'four'), (5, 'five'), (6, 'six'),
                 (7, 'seven');

             -- ModerateMessage, the only action that was already flat.
             INSERT INTO moderation_actions (id, rule_id, type) VALUES (1, 1, 'ModerateMessage');
             INSERT INTO moderation_action__moderate_message (action_id) VALUES (1);

             -- KickAuthor, all three message settings.
             INSERT INTO moderation_actions (id, rule_id, type) VALUES
                 (2, 2, 'KickAuthor'), (3, 3, 'KickAuthor'), (4, 4, 'KickAuthor');
             INSERT INTO moderation_action__kick_author (action_id, delete_messages) VALUES
                 (2, 0), (3, 1), (4, 2);

             -- SetAuthorObserver, both message settings.
             INSERT INTO moderation_actions (id, rule_id, type) VALUES
                 (5, 5, 'SetAuthorObserver'), (6, 6, 'SetAuthorObserver');
             INSERT INTO moderation_action__set_author_observer (action_id, delete_message) VALUES
                 (5, 0), (6, 1);

             -- A value outside the three the writer produced. The reader being
             -- replaced treated anything but 0 and 2 as \"also moderate the
             -- message\", so the migration has to as well.
             INSERT INTO moderation_actions (id, rule_id, type) VALUES (7, 7, 'KickAuthor');
             INSERT INTO moderation_action__kick_author (action_id, delete_messages)
                  VALUES (7, 7);",
        )
        .unwrap();

        apply(&mut conn).unwrap();

        let conn = Arc::new(Mutex::new(conn));
        let repo =
            crate::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository::new(
                conn.clone(),
            );
        use crate::domain::moderator::ports::{ModerationAction, ModerationRepository};
        let loaded = repo.get_group_rules(&11).await.unwrap();
        let actions: Vec<Vec<ModerationAction>> =
            loaded.into_iter().map(|owned| owned.rule.actions).collect();

        use ModerationAction::*;
        assert_eq!(
            actions,
            vec![
                // ModerateMessage is unchanged.
                vec![ModerateMessage],
                // KickAuthor / None: nothing happens to the messages.
                vec![KickAuthor {
                    delete_all_messages: false
                }],
                // KickAuthor / TriggeredMessage: the message deletion becomes its
                // own action, running before the kick.
                vec![
                    ModerateMessage,
                    KickAuthor {
                        delete_all_messages: false
                    }
                ],
                // KickAuthor / AllMessages: the kick keeps doing the deleting.
                vec![KickAuthor {
                    delete_all_messages: true
                }],
                // SetAuthorObserver / None.
                vec![SetAuthorObserver],
                // SetAuthorObserver / TriggeredMessage: observer first, then the
                // message is moderated.
                vec![SetAuthorObserver, ModerateMessage],
                // An out-of-range setting keeps the meaning the old reader gave
                // it, which was the same as TriggeredMessage.
                vec![
                    ModerateMessage,
                    KickAuthor {
                        delete_all_messages: false
                    }
                ],
            ]
        );

        let guard = conn.lock().unwrap();
        // The three split rules gained an action each; nothing else did.
        let action_count: i64 = guard
            .query_row("SELECT COUNT(*) FROM moderation_actions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(action_count, 10);
        // Every action belongs to a rule and has its settings row, including the
        // two rows the backfill created.
        let unowned: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM moderation_actions WHERE rule_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unowned, 0);
        let moderate_settings: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM moderation_action__moderate_message",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(moderate_settings, 4);
        let temp_tables: i64 = guard
            .query_row("SELECT COUNT(*) FROM sqlite_temp_master", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            temp_tables, 0,
            "the migration's scratch tables should be gone"
        );
    }

    /// The invariant the schema is shaped around: the group is the root, so one
    /// delete empties everything, with no manual sweeping anywhere.
    #[tokio::test]
    async fn test_deleting_a_group_row_cascades_through_the_whole_schema() {
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        run(conn.clone()).await.unwrap();

        let repo =
            crate::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository::new(
                conn.clone(),
            );
        use crate::domain::moderator::ports::{
            ModerationAction, ModerationCondition, ModerationRepository, ModerationRule,
        };
        let group_id = repo.save_owner(&800, "Cascade Group", &80).await.unwrap();
        repo.set_group_rules(
            &group_id,
            &[ModerationRule {
                actions: vec![ModerationAction::ModerateMessage],
                condition: ModerationCondition::All {
                    conditions: vec![
                        ModerationCondition::ContainsWords {
                            keywords: vec!["alpha".to_string()],
                        },
                        ModerationCondition::Not {
                            condition: Box::new(ModerationCondition::MatchesRegex {
                                patterns: vec!["z".to_string()],
                            }),
                        },
                        ModerationCondition::ContainsRepeatedSequence {
                            min_repeats: 5,
                            min_length: 1,
                        },
                        ModerationCondition::AuthorJoinedRecently {
                            time_window_minutes: 10,
                        },
                    ],
                },
            }],
        )
        .await
        .unwrap();

        let guard = conn.lock().unwrap();
        guard
            .execute(
                "DELETE FROM moderation_groups WHERE group_id = ?1",
                [group_id],
            )
            .unwrap();

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
        for table in tables {
            let count: i64 = guard
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                count, 0,
                "{table} should be empty after the group is deleted"
            );
        }
    }
}
