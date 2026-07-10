use async_trait::async_trait;
use rand::RngExt;
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};

use crate::domain::moderator::ports::{
    Err, Group, GroupId, MessengerGroupId, ModerationRepository, ModerationRule,
    OwnedModerationRule, UserId,
};
const GROUP_ID_MIN: i64 = 1;
const GROUP_ID_MAX: i64 = 1_000_000;
const GROUP_ID_ALLOC_MAX_ATTEMPTS: usize = 32;

/// Maximum number of keywords allowed per group.
const MAX_KEYWORDS_PER_GROUP: usize = 10_000;

/// Maximum length (in characters) allowed for a single keyword.
const MAX_KEYWORD_LENGTH: usize = 100;

/// Maximum number of messages allowed per group.
const MAX_MESSAGES_PER_GROUP: usize = 10_000;

/// Maximum length (in characters) allowed for a single message in the messages blacklist.
const MAX_MESSAGE_LENGTH: usize = 1000;

#[derive(Clone)]
pub struct SqliteModerationRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteModerationRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl ModerationRepository for SqliteModerationRepository {
    async fn save_owner(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
        owner_id: &UserId,
    ) -> Result<GroupId, Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        let oid = *owner_id;
        let name = name.to_string();
        tokio::task::spawn_blocking(move || -> Result<GroupId, Err> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut rng = rand::rng();
            for _ in 0..GROUP_ID_ALLOC_MAX_ATTEMPTS {
                let gid: i64 = rng.random_range(GROUP_ID_MIN..GROUP_ID_MAX);
                let res = guard.execute(
                    "INSERT INTO moderation_groups (group_id, messenger_group_id, owner_id, group_name)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![gid, mid, oid, name],
                );
                match res {
                    Ok(_) => return Ok(gid),
                    Err(rusqlite::Error::SqliteFailure(e, _))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        // group_id collision: try a new random id
                        continue;
                    }
                    Err(e) => return Err(e.to_string().into()),
                }
            }
            Err("failed to allocate a unique group_id".into())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
    }

    async fn get_owner_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<UserId>, Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || -> Result<Option<i64>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt = guard
                .prepare("SELECT owner_id FROM moderation_groups WHERE messenger_group_id = ?1")?;
            let mut rows = stmt.query(params![mid])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn get_owner_by_id(&self, group_id: &GroupId) -> Result<Option<UserId>, Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        tokio::task::spawn_blocking(move || -> Result<Option<i64>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt =
                guard.prepare("SELECT owner_id FROM moderation_groups WHERE group_id = ?1")?;
            let mut rows = stmt.query(params![gid])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err> {
        let conn = self.conn.clone();
        let oid = *owner_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<Group>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt = guard.prepare(
                "SELECT group_id, group_name, notifications_enabled, dry_mode_enabled FROM moderation_groups WHERE owner_id = ?1",
            )?;
            let rows = stmt.query_map(params![oid], |row| {
                Ok(Group {
                    id: row.get::<_, i64>(0)?,
                    owner_id: oid,
                    name: row.get::<_, String>(1)?,
                    notifications_enabled: row.get::<_, i64>(2)? != 0,
                    dry_mode_enabled: row.get::<_, i64>(3)? != 0,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn set_group_name(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
    ) -> Result<(), Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        let name_owned = name.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            guard.execute(
                "UPDATE moderation_groups SET group_name = ?2 WHERE messenger_group_id = ?1",
                rusqlite::params![mid, name_owned],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn get_group_rules(&self, group_id: &GroupId) -> Result<Vec<OwnedModerationRule>, Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        tokio::task::spawn_blocking(move || {
            crate::infrastructure::adapters::moderator_repo_sqlite_rules::load_rules_for_group(
                &conn, gid,
            )
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
    }

    async fn get_group_rules_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || {
            let gid = {
                let guard = conn.lock().expect("moderation repo connection poisoned");
                let mut stmt = guard.prepare("SELECT group_id FROM moderation_groups WHERE messenger_group_id = ?1")?;
                stmt.query_row(params![mid], |row| row.get::<_, i64>(0))
            };
            match gid {
                Ok(gid) => crate::infrastructure::adapters::moderator_repo_sqlite_rules::load_rules_for_group(&conn, gid),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
                Err(e) => Err(e.into()),
            }
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
    }

    async fn set_group_rules(
        &self,
        group_id: &GroupId,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let conn = self.conn.clone();
        let gid = *group_id;

        let mut rules = rules.to_vec();
        for rule in &mut rules {
            match rule {
                ModerationRule::WordsBlacklist { keywords } => {
                    keywords.sort();
                    keywords.dedup();
                    if keywords.len() > MAX_KEYWORDS_PER_GROUP {
                        return Err(format!(
                            "Too many keywords: {} provided, maximum is {}",
                            keywords.len(),
                            MAX_KEYWORDS_PER_GROUP
                        )
                        .into());
                    }
                    if let Some(keyword) = keywords
                        .iter()
                        .find(|k| k.chars().count() > MAX_KEYWORD_LENGTH)
                    {
                        return Err(format!(
                            "Keyword too long: {} characters, maximum is {}",
                            keyword.chars().count(),
                            MAX_KEYWORD_LENGTH
                        )
                        .into());
                    }
                }
                ModerationRule::MessagesBlacklist {
                    messages,
                    case_sensitive: _,
                } => {
                    messages.sort();
                    messages.dedup();
                    if messages.len() > MAX_MESSAGES_PER_GROUP {
                        return Err(format!(
                            "Too many messages: {} provided, maximum is {}",
                            messages.len(),
                            MAX_MESSAGES_PER_GROUP
                        )
                        .into());
                    }
                    if let Some(msg) = messages
                        .iter()
                        .find(|d| d.chars().count() > MAX_MESSAGE_LENGTH)
                    {
                        return Err(format!(
                            "Message too long: {} characters, maximum is {}",
                            msg.chars().count(),
                            MAX_MESSAGE_LENGTH,
                        )
                        .into());
                    }
                }
                ModerationRule::LinksBlacklist { blocked } => {
                    blocked.sort();
                    blocked.dedup();
                    if blocked.len() > MAX_KEYWORDS_PER_GROUP {
                        return Err(format!(
                            "Too many domains: {} provided, maximum is {}",
                            blocked.len(),
                            MAX_KEYWORDS_PER_GROUP
                        )
                        .into());
                    }
                    if let Some(domain) = blocked
                        .iter()
                        .find(|d| d.chars().count() > MAX_KEYWORD_LENGTH)
                    {
                        return Err(format!(
                            "Domain too long: {} characters, maximum is {}",
                            domain.chars().count(),
                            MAX_KEYWORD_LENGTH
                        )
                        .into());
                    }
                }
                ModerationRule::LinksWhitelist { allowed } => {
                    allowed.sort();
                    allowed.dedup();
                    if allowed.len() > MAX_KEYWORDS_PER_GROUP {
                        return Err(format!(
                            "Too many domains: {} provided, maximum is {}",
                            allowed.len(),
                            MAX_KEYWORDS_PER_GROUP
                        )
                        .into());
                    }
                    if let Some(domain) = allowed
                        .iter()
                        .find(|d| d.chars().count() > MAX_KEYWORD_LENGTH)
                    {
                        return Err(format!(
                            "Domain too long: {} characters, maximum is {}",
                            domain.chars().count(),
                            MAX_KEYWORD_LENGTH
                        )
                        .into());
                    }
                }
                ModerationRule::LinksWhitelistTop100 { allowed: _ } => {}
            }
        }

        tokio::task::spawn_blocking(move || -> Result<(), Err> {
            let mut guard = conn.lock().expect("moderation repo connection poisoned");
            let tx = guard.transaction().map_err(|e| -> Err { e.to_string().into() })?;

            // Delete all previous rules for this group. Child rows (keywords /
            // domains) are removed automatically via ON DELETE CASCADE.
            for table in [
                "moderation_rule__words_blacklist",
                "moderation_rule__messages_blacklist",
                "moderation_rule__links_blacklist",
                "moderation_rule__links_whitelist",
                "moderation_rule__links_whitelist_top100",
            ] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE group_id = ?1"),
                    rusqlite::params![gid],
                )
                .map_err(|e| -> Err { e.to_string().into() })?;
            }

            for (rank, rule) in rules.into_iter().enumerate() {
                let rank = rank as i64;
                match rule {
                    ModerationRule::WordsBlacklist { keywords } => {
                        tx.execute(
                            "INSERT INTO moderation_rule__words_blacklist (group_id, rank) VALUES (?1, ?2)",
                            rusqlite::params![gid, rank],
                        )
                        .map_err(|e| -> Err { e.to_string().into() })?;
                        let rule_id = tx.last_insert_rowid();
                        let mut stmt = tx
                            .prepare("INSERT INTO moderation_rule__words_blacklist__keywords (rule_id, keyword) VALUES (?1, ?2)")
                            .map_err(|e| -> Err { e.to_string().into() })?;
                        for kw in keywords.iter().filter(|k| !k.is_empty()) {
                            stmt.execute(rusqlite::params![rule_id, kw])
                                .map_err(|e| -> Err { e.to_string().into() })?;
                        }
                    }
                    ModerationRule::MessagesBlacklist { messages, case_sensitive } => {
                        tx.execute(
                            "INSERT INTO moderation_rule__messages_blacklist (group_id, rank, case_sensitive) VALUES (?1, ?2, ?3)",
                            rusqlite::params![gid, rank, case_sensitive],
                        )
                        .map_err(|e| -> Err { e.to_string().into() })?;
                        let rule_id = tx.last_insert_rowid();
                        let mut stmt = tx
                            .prepare("INSERT INTO moderation_rule__messages_blacklist__messages (rule_id, message) VALUES (?1, ?2)")
                            .map_err(|e| -> Err { e.to_string().into() })?;
                        for msg in messages.iter().filter(|k| !k.is_empty()) {
                            stmt.execute(rusqlite::params![rule_id, msg])
                                .map_err(|e| -> Err { e.to_string().into() })?;
                        }
                    }
                    ModerationRule::LinksBlacklist { blocked } => {
                        tx.execute(
                            "INSERT INTO moderation_rule__links_blacklist (group_id, rank) VALUES (?1, ?2)",
                            rusqlite::params![gid, rank],
                        )
                        .map_err(|e| -> Err { e.to_string().into() })?;
                        let rule_id = tx.last_insert_rowid();
                        let mut stmt = tx
                            .prepare("INSERT INTO moderation_rule__links_blacklist__domains (rule_id, domain) VALUES (?1, ?2)")
                            .map_err(|e| -> Err { e.to_string().into() })?;
                        for domain in blocked {
                            stmt.execute(rusqlite::params![rule_id, domain])
                                .map_err(|e| -> Err { e.to_string().into() })?;
                        }
                    }
                    ModerationRule::LinksWhitelist { allowed } => {
                        tx.execute(
                            "INSERT INTO moderation_rule__links_whitelist (group_id, rank) VALUES (?1, ?2)",
                            rusqlite::params![gid, rank],
                        )
                        .map_err(|e| -> Err { e.to_string().into() })?;
                        let rule_id = tx.last_insert_rowid();
                        let mut stmt = tx
                            .prepare("INSERT INTO moderation_rule__links_whitelist__domains (rule_id, domain) VALUES (?1, ?2)")
                            .map_err(|e| -> Err { e.to_string().into() })?;
                        for domain in allowed {
                            stmt.execute(rusqlite::params![rule_id, domain])
                                .map_err(|e| -> Err { e.to_string().into() })?;
                        }
                    }
                    ModerationRule::LinksWhitelistTop100 { allowed } => {
                        tx.execute(
                            "INSERT INTO moderation_rule__links_whitelist_top100 (group_id, rank) VALUES (?1, ?2)",
                            rusqlite::params![gid, rank],
                        )
                        .map_err(|e| -> Err { e.to_string().into() })?;
                        let rule_id = tx.last_insert_rowid();
                        let mut stmt = tx
                            .prepare("INSERT INTO moderation_rule__links_whitelist_top100__allowed (rule_id, domain) VALUES (?1, ?2)")
                            .map_err(|e| -> Err { e.to_string().into() })?;
                        for a in allowed.iter().filter(|k| !k.is_empty()) {
                            stmt.execute(rusqlite::params![rule_id, a])
                                .map_err(|e| -> Err { e.to_string().into() })?;
                        }
                    }
                }
            }

            tx.commit().map_err(|e| -> Err { e.to_string().into() })?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
    }

    async fn delete_group_data(&self, messenger_group_id: &MessengerGroupId) -> Result<(), Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            // ON DELETE CASCADE takes care of everything.
            guard.execute(
                "DELETE FROM moderation_groups WHERE messenger_group_id = ?1",
                params![mid],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn get_group_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<Group>, Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || -> Result<Option<Group>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt = guard.prepare(
                "SELECT group_id, owner_id, group_name, notifications_enabled, dry_mode_enabled
                 FROM moderation_groups WHERE messenger_group_id = ?1",
            )?;
            let mut rows = stmt.query(params![mid])?;
            if let Some(row) = rows.next()? {
                Ok(Some(Group {
                    id: row.get(0)?,
                    owner_id: row.get(1)?,
                    name: row.get(2)?,
                    notifications_enabled: row.get::<_, i64>(3)? != 0,
                    dry_mode_enabled: row.get::<_, i64>(4)? != 0,
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn set_notifications_enabled(
        &self,
        group_id: &GroupId,
        enabled: bool,
    ) -> Result<(), Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        let value: i64 = if enabled { 1 } else { 0 };
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            guard.execute(
                "UPDATE moderation_groups SET notifications_enabled = ?2 WHERE group_id = ?1",
                params![gid, value],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn set_dry_mode_enabled(&self, group_id: &GroupId, enabled: bool) -> Result<(), Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        let value: i64 = if enabled { 1 } else { 0 };
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            guard.execute(
                "UPDATE moderation_groups SET dry_mode_enabled = ?2 WHERE group_id = ?1",
                params![gid, value],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }
}
