use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::domain::moderator::ports::{Err, GroupCharacterActivityRepository, MessengerGroupId};
use crate::infrastructure::drivers::sliding_window_counter::SlidingWindowCounter;

/// Hard ceiling: no character record is ever retained in memory longer than 60
/// minutes. Retention is this adapter's policy, not the counter's.
pub const MAX_RETENTION_DURATION: Duration = Duration::from_secs(60 * 60);

/// In-memory implementation of `GroupCharacterActivityRepository`: one counter per group,
/// shared by all its members.
///
/// A message weighs as many characters as it has, so the counter's total is the
/// number of characters the group's members wrote in the window.
#[derive(Default)]
pub struct InMemoryGroupCharacterActivityRepository {
    counter: SlidingWindowCounter<MessengerGroupId>,
}

impl InMemoryGroupCharacterActivityRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Explicitly sweep and purge all records older than `now`, removing empty
    /// group entries. Returns the number of group entries removed.
    pub fn purge_expired(&self, now: DateTime<Utc>) -> Result<usize, Err> {
        self.counter.purge_expired(now)
    }

    /// Number of active group keys currently tracked in memory.
    #[cfg(test)]
    pub fn active_key_count(&self) -> usize {
        self.counter.active_key_count().unwrap()
    }
}

#[async_trait]
impl GroupCharacterActivityRepository for InMemoryGroupCharacterActivityRepository {
    async fn record_characters(
        &self,
        group_id: &MessengerGroupId,
        timestamp: DateTime<Utc>,
        character_count: u32,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.counter.add(
            *group_id,
            timestamp,
            character_count,
            ttl.min(MAX_RETENTION_DURATION),
        )
    }

    async fn sum_characters_since(
        &self,
        group_id: &MessengerGroupId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        self.counter.total(group_id, since, now)
    }
}

#[cfg(test)]
mod tests;
