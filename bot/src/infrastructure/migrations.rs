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
        assert_eq!(version, 24);
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
            DeleteAuthorMessages, ModerationAction, ModerationCondition, ModerationRepository,
            ModerationRule,
        };
        assert_eq!(
            loaded,
            vec![
                ModerationRule {
                    action: ModerationAction::ModerateMessage,
                    condition: ModerationCondition::ContainsBannedWords {
                        keywords: vec!["alpha".to_string(), "beta".to_string()],
                    },
                },
                ModerationRule {
                    action: ModerationAction::KickAuthor {
                        delete_messages: DeleteAuthorMessages::AllMessages,
                    },
                    condition: ModerationCondition::MatchesExactMessage {
                        messages: vec!["spam".to_string()],
                        case_sensitive: true,
                    },
                },
                ModerationRule {
                    action: ModerationAction::ModerateMessage,
                    condition: ModerationCondition::UserExceedsMessagesRateLimit {
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
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::All {
                    conditions: vec![
                        ModerationCondition::ContainsBannedWords {
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
                        ModerationCondition::UserJoinedRecently {
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
