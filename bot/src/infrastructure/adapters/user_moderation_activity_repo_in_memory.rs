use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;

use super::user_activity_repo_in_memory::InMemoryUserActivityRepository;
use crate::domain::moderator::ports::{
    Err, MessengerGroupId, UserActivityRepository, UserId, UserModerationActivityRepository,
};

/// In-memory implementation of `UserModerationActivityRepository`.
/// Reuses `InMemoryUserActivityRepository` under the hood to store and evict
/// timestamps of messages moderated by the bot.
#[derive(Clone)]
pub struct InMemoryUserModerationActivityRepository {
    storage: Arc<InMemoryUserActivityRepository>,
}

impl Default for InMemoryUserModerationActivityRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryUserModerationActivityRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(InMemoryUserActivityRepository::new()),
        }
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
        self.storage
            .record_user_message(group_id, user_id, timestamp, ttl)
            .await
    }

    async fn count_moderated_messages_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        self.storage
            .count_messages_since(group_id, user_id, since, now)
            .await
    }
}

#[cfg(test)]
mod tests;
