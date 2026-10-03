use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use crate::domain::moderator::ports::{
    Err, GroupMessageHistoryRepository, MessageId, MessengerGroupId, RecentGroupMessage,
};

/// Hard ceiling: no message is kept in memory longer than 24 hours after it
/// was posted or last edited, however few the group has received since — a
/// message from yesterday is rarely context, and it is a member's text kept
/// for nothing. Retention is this adapter's policy, not the caller's.
pub const MAX_RETENTION: chrono::Duration = chrono::Duration::hours(24);

/// Hard ceiling on the messages kept per group, whatever `keep` the caller
/// asks for — it bounds the memory one group can take.
pub const MAX_KEPT_PER_GROUP: u32 = 50;

/// In-memory implementation of `GroupMessageHistoryRepository`: per group, its
/// latest messages, oldest first. Lost on restart, which costs the first
/// messages after one their context, and nothing else.
#[derive(Default)]
pub struct InMemoryGroupMessageHistoryRepository {
    groups: Mutex<HashMap<MessengerGroupId, VecDeque<RecentGroupMessage>>>,
}

impl InMemoryGroupMessageHistoryRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn groups(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<MessengerGroupId, VecDeque<RecentGroupMessage>>>, Err> {
        self.groups.lock().map_err(|e| -> Err {
            format!("InMemoryGroupMessageHistoryRepository mutex poisoned: {e}").into()
        })
    }

    /// Drop every message past retention at `now`, removing groups left with
    /// none. Returns the number of groups removed.
    pub fn purge_expired(&self, now: DateTime<Utc>) -> Result<usize, Err> {
        let mut groups = self.groups()?;
        let before = groups.len();
        groups.retain(|_, messages| {
            messages.retain(|message| !expired(message, now));
            !messages.is_empty()
        });
        Ok(before - groups.len())
    }

    /// Number of groups currently tracked in memory.
    #[cfg(test)]
    pub fn active_group_count(&self) -> usize {
        self.groups().unwrap().len()
    }
}

fn expired(message: &RecentGroupMessage, now: DateTime<Utc>) -> bool {
    now - message.timestamp >= MAX_RETENTION
}

#[async_trait]
impl GroupMessageHistoryRepository for InMemoryGroupMessageHistoryRepository {
    async fn record_message(
        &self,
        group_id: &MessengerGroupId,
        message: RecentGroupMessage,
        keep: u32,
    ) -> Result<(), Err> {
        let keep = keep.min(MAX_KEPT_PER_GROUP) as usize;
        let mut groups = self.groups()?;
        let messages = groups.entry(*group_id).or_default();
        let now = message.timestamp;
        messages.retain(|kept| !expired(kept, now));
        messages.push_back(message);
        while messages.len() > keep {
            messages.pop_front();
        }
        if messages.is_empty() {
            groups.remove(group_id);
        }
        Ok(())
    }

    async fn record_edit(
        &self,
        group_id: &MessengerGroupId,
        message: RecentGroupMessage,
    ) -> Result<(), Err> {
        let mut groups = self.groups()?;
        if let Some(kept) = groups.get_mut(group_id).and_then(|messages| {
            messages
                .iter_mut()
                .find(|m| m.message_id == message.message_id)
        }) {
            *kept = message;
        }
        Ok(())
    }

    async fn forget_message(
        &self,
        group_id: &MessengerGroupId,
        message_id: &MessageId,
    ) -> Result<(), Err> {
        let mut groups = self.groups()?;
        if let Some(messages) = groups.get_mut(group_id) {
            messages.retain(|m| m.message_id != *message_id);
            if messages.is_empty() {
                groups.remove(group_id);
            }
        }
        Ok(())
    }

    async fn messages_before(
        &self,
        group_id: &MessengerGroupId,
        message_id: &MessageId,
        count: u32,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecentGroupMessage>, Err> {
        let groups = self.groups()?;
        let Some(messages) = groups.get(group_id) else {
            return Ok(Vec::new());
        };
        let end = messages
            .iter()
            .position(|m| m.message_id == *message_id)
            .unwrap_or(messages.len());
        let earlier: Vec<&RecentGroupMessage> =
            messages.range(..end).filter(|m| !expired(m, now)).collect();
        let skip = earlier.len().saturating_sub(count as usize);
        Ok(earlier.into_iter().skip(skip).cloned().collect())
    }
}

#[cfg(test)]
mod tests;
