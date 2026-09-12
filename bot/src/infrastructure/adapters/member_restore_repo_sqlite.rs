//! SQLite persistence for the member restores the bot still owes.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};

use crate::domain::moderator::ports::{
    Err, MemberRestoreRepository, MessengerGroupId, ScheduledMemberRestore, UserId,
};

#[cfg(test)]
mod tests;

pub struct SqliteMemberRestoreRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteMemberRestoreRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl MemberRestoreRepository for SqliteMemberRestoreRepository {
    async fn save(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
        execute_at: DateTime<Utc>,
    ) -> Result<(), Err> {
        let conn = self.conn.clone();
        let m_gid = *messenger_group_id;
        let member_id = *member_id;
        let execute_at = execute_at.timestamp();

        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("member restore connection poisoned");
            // The group is looked up rather than passed in, so an unregistered
            // group simply stores nothing to restore.
            guard.execute(
                "INSERT INTO moderation_set_author_observer_restores (group_id, member_id, execute_at)
                     SELECT group_id, ?2, ?3 FROM moderation_groups WHERE messenger_group_id = ?1
                 ON CONFLICT (group_id, member_id) DO UPDATE SET execute_at = excluded.execute_at",
                params![m_gid, member_id, execute_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn delete_for_member(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
    ) -> Result<(), Err> {
        let conn = self.conn.clone();
        let m_gid = *messenger_group_id;
        let member_id = *member_id;

        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("member restore connection poisoned");
            guard.execute(
                "DELETE FROM moderation_set_author_observer_restores
                       WHERE member_id = ?2
                         AND group_id = (SELECT group_id FROM moderation_groups
                                          WHERE messenger_group_id = ?1)",
                params![m_gid, member_id],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn list_due(&self, now: DateTime<Utc>) -> Result<Vec<ScheduledMemberRestore>, Err> {
        let conn = self.conn.clone();
        let now = now.timestamp();

        tokio::task::spawn_blocking(
            move || -> Result<Vec<ScheduledMemberRestore>, rusqlite::Error> {
                let guard = conn.lock().expect("member restore connection poisoned");
                let mut stmt = guard.prepare(
                    "SELECT r.id, g.messenger_group_id, r.member_id, r.execute_at
                       FROM moderation_set_author_observer_restores r
                       JOIN moderation_groups g ON g.group_id = r.group_id
                      WHERE r.execute_at <= ?1
                      ORDER BY r.execute_at",
                )?;
                let rows = stmt.query_map(params![now], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;

                let mut out = Vec::new();
                for row in rows {
                    let (id, messenger_group_id, member_id, execute_at) = row?;
                    let Some(execute_at) = DateTime::from_timestamp(execute_at, 0) else {
                        log::warn!("unreadable execute_at on restore {id}, skipping it");
                        continue;
                    };
                    out.push(ScheduledMemberRestore {
                        id,
                        messenger_group_id,
                        member_id,
                        execute_at,
                    });
                }
                Ok(out)
            },
        )
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }

    async fn delete(&self, id: i64, execute_at: DateTime<Utc>) -> Result<(), Err> {
        let conn = self.conn.clone();
        let execute_at = execute_at.timestamp();

        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let guard = conn.lock().expect("member restore connection poisoned");
            guard.execute(
                "DELETE FROM moderation_set_author_observer_restores
                       WHERE id = ?1 AND execute_at = ?2",
                params![id, execute_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| -> Err { e.to_string().into() })?
        .map_err(|e| -> Err { e.to_string().into() })
    }
}
