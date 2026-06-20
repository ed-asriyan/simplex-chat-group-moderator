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
        tokio::task::spawn_blocking(move || crate::infrastructure::adapters::moderator_repo_sqlite_rules::load_rules_for_group(&conn, gid))
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
                let guard = conn.lock().unwrap();
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
                    if let Some(domain) = blocked.iter().find(|d| d.chars().count() > MAX_KEYWORD_LENGTH) {
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
                    if let Some(domain) = allowed.iter().find(|d| d.chars().count() > MAX_KEYWORD_LENGTH) {
                        return Err(format!(
                            "Domain too long: {} characters, maximum is {}",
                            domain.chars().count(),
                            MAX_KEYWORD_LENGTH
                        )
                        .into());
                    }
                }
                ModerationRule::LinksWhitelistTop100 {} => {}
            }
        }

        tokio::task::spawn_blocking(move || -> Result<(), Err> {
             let mut guard = conn.lock().unwrap();
             let tx = guard.transaction().map_err(|e| -> Err { e.to_string().into() })?;

             // Delete all previous rules
             tx.execute("DELETE FROM moderation_rules WHERE group_id = ?1", rusqlite::params![gid]).map_err(|e| -> Err { e.to_string().into() })?;
             
             let mut rng = rand::rng();
             for rule in rules {
                 let rule_type_str = match rule {
                     ModerationRule::WordsBlacklist { .. } => "words_blacklist",
                     ModerationRule::LinksBlacklist { .. } => "links_blacklist",
                     ModerationRule::LinksWhitelist { .. } => "links_whitelist",
                     ModerationRule::LinksWhitelistTop100 {} => "links_whitelist_top100",
                 };

                 let mut rule_id: i64 = 0;
                 for _ in 0..GROUP_ID_ALLOC_MAX_ATTEMPTS {
                     let id_candidate: i64 = rng.random_range(GROUP_ID_MIN..GROUP_ID_MAX);
                     let res = tx.execute(
                         "INSERT INTO moderation_rules (id, group_id, rule_type) VALUES (?1, ?2, ?3)",
                         rusqlite::params![id_candidate, gid, rule_type_str]
                     );
                     match res {
                         Ok(_) => {
                             rule_id = id_candidate;
                             break;
                         }
                         Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                             continue;
                         }
                         Err(e) => return Err(e.to_string().into()),
                     }
                 }

                 if rule_id == 0 {
                     return Err("failed to allocate a unique rule_id".into());
                 }

                 match rule {
                     ModerationRule::WordsBlacklist { keywords: k } => {
                          let mut stmt = tx.prepare("INSERT INTO moderation_rule_keywords (rule_id, keyword) VALUES (?1, ?2)").map_err(|e| -> Err { e.to_string().into() })?;
                          for kw in k.iter().filter(|k| !k.is_empty()) {
                              stmt.execute(rusqlite::params![rule_id, kw]).map_err(|e| -> Err { e.to_string().into() })?;
                          }
                     },
                     ModerationRule::LinksBlacklist { blocked } => {
                     let mut stmt = tx.prepare("INSERT INTO moderation_rule_link_domains (rule_id, domain) VALUES (?1, ?2)").map_err(|e| -> Err { e.to_string().into() })?;
                     for domain in blocked {
                         stmt.execute(rusqlite::params![rule_id, domain]).map_err(|e| -> Err { e.to_string().into() })?;
                     }
                 },
                 ModerationRule::LinksWhitelist { allowed } => {
                     let mut stmt = tx.prepare("INSERT INTO moderation_rule_link_domains (rule_id, domain) VALUES (?1, ?2)").map_err(|e| -> Err { e.to_string().into() })?;
                     for domain in allowed {
                         stmt.execute(rusqlite::params![rule_id, domain]).map_err(|e| -> Err { e.to_string().into() })?;
                     }
                 }
                 ModerationRule::LinksWhitelistTop100 {} => {
                     // No domain data to store — the list is built into the binary.
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
