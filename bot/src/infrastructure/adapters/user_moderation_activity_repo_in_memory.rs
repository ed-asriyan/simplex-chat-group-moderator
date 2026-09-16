use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::domain::moderator::ports::{
    Err, MessengerGroupId, UserId, UserModerationActivityRepository,
};
use crate::infrastructure::drivers::sliding_window_counter::SlidingWindowCounter;

/// Hard ceiling: no moderated-message record is ever retained in memory longer
/// than 60 minutes. Retention is this adapter's policy, not the counter's.
pub const MAX_RETENTION_DURATION: Duration = Duration::from_secs(60 * 60);

/// In-memory implementation of `UserModerationActivityRepository`.
///
/// Every moderated message weighs 1, so the counter's total *is* the count.
#[derive(Default)]
pub struct InMemoryUserModerationActivityRepository {
    counter: SlidingWindowCounter<(MessengerGroupId, UserId)>,
}

impl InMemoryUserModerationActivityRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UserModerationActivityRepository for InMemoryUserModerationActivityRepository {
    async fn record_moderated_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.counter.add(
            (*group_id, *user_id),
            timestamp,
            1,
            ttl.min(MAX_RETENTION_DURATION),
        )
    }

    async fn count_moderated_messages_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        self.counter.total(&(*group_id, *user_id), since, now)
    }
}

#[cfg(test)]
mod tests;
