use async_trait::async_trait;
use rand::RngExt;
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};

use crate::domain::moderator::ports::{
    Err, Group, GroupId, MessengerGroupId, ModerationRepository, UserId,
};

const GROUP_ID_MIN: i64 = 1;
const GROUP_ID_MAX: i64 = 1_000_000;
const GROUP_ID_ALLOC_MAX_ATTEMPTS: usize = 32;

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

    async fn save_keywords(&self, group_id: &GroupId, keywords: Vec<String>) -> Result<(), Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let mut guard = conn.lock().expect("moderation repo connection poisoned");
            let tx = guard.transaction()?;
            tx.execute(
                "DELETE FROM moderation_group_keywords WHERE group_id = ?1",
                params![gid],
            )?;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR IGNORE INTO moderation_group_keywords (group_id, keyword) VALUES (?1, ?2)",
                )?;
                for kw in keywords.iter().filter(|k| !k.is_empty()) {
                    stmt.execute(params![gid, kw])?;
                }
            }
            tx.commit()?;
            Ok(())
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
        let name = name.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            guard.execute(
                "UPDATE moderation_groups SET group_name = ?2 WHERE messenger_group_id = ?1",
                params![mid, name],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn get_keywords(&self, group_id: &GroupId) -> Result<Vec<String>, Err> {
        let conn = self.conn.clone();
        let gid = *group_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<String>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt = guard
                .prepare("SELECT keyword FROM moderation_group_keywords WHERE group_id = ?1")?;
            let rows = stmt.query_map(params![gid], |row| row.get::<_, String>(0))?;
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

    async fn get_keywords_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Vec<String>, Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<String>, rusqlite::Error> {
            let guard = conn.lock().expect("moderation repo connection poisoned");
            let mut stmt = guard.prepare(
                "SELECT k.keyword
                 FROM moderation_group_keywords k
                 JOIN moderation_groups g ON g.group_id = k.group_id
                 WHERE g.messenger_group_id = ?1",
            )?;
            let rows = stmt.query_map(params![mid], |row| row.get::<_, String>(0))?;
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

    async fn delete_group_data(&self, messenger_group_id: &MessengerGroupId) -> Result<(), Err> {
        let conn = self.conn.clone();
        let mid = *messenger_group_id;
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let mut guard = conn.lock().expect("moderation repo connection poisoned");
            let tx = guard.transaction()?;
            tx.execute(
                "DELETE FROM moderation_group_keywords
                 WHERE group_id IN (
                     SELECT group_id FROM moderation_groups WHERE messenger_group_id = ?1
                 )",
                params![mid],
            )?;
            tx.execute(
                "DELETE FROM moderation_groups WHERE messenger_group_id = ?1",
                params![mid],
            )?;
            tx.commit()?;
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
