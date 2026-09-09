use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use crate::domain::moderator::ports::{Err, MessengerGroupId, UserActivityRepository, UserId};

/// Hard ceiling: no message record is ever retained in memory longer than 60 minutes.
pub const MAX_RETENTION_DURATION: Duration = Duration::from_secs(60 * 60);

/// Minimum interval between full sweeps of the activity map.
const SWEEP_INTERVAL: chrono::Duration = chrono::Duration::minutes(5);

#[derive(Clone, Debug)]
struct MessageRecord {
    timestamp: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

/// In-memory implementation of `UserActivityRepository`.
/// Stores user message timestamps in memory with TTL-based automatic expiration.
/// Guaranteed to never retain records or inactive user entries older than MAX_RETENTION_DURATION.
pub struct InMemoryUserActivityRepository {
    state: Mutex<HashMap<(MessengerGroupId, UserId), VecDeque<MessageRecord>>>,
    last_sweep: Mutex<Option<DateTime<Utc>>>,
}

impl Default for InMemoryUserActivityRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryUserActivityRepository {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            last_sweep: Mutex::new(None),
        }
    }

    /// Explicitly sweep and purge all records older than `now`, removing empty user entries.
    /// Returns the number of empty user entries removed from the map.
    pub fn purge_expired(&self, now: DateTime<Utc>) -> Result<usize, Err> {
        let mut guard = self.state.lock().map_err(|e| -> Err {
            format!("InMemoryUserActivityRepository mutex poisoned: {e}").into()
        })?;
        if let Ok(mut last) = self.last_sweep.lock() {
            *last = Some(now);
        }
        let before_count = guard.len();
        guard.retain(|_, deque| {
            while let Some(front) = deque.front() {
                if front.expires_at <= now {
                    deque.pop_front();
                } else {
                    break;
                }
            }
            !deque.is_empty()
        });
        Ok(before_count.saturating_sub(guard.len()))
    }

    /// Number of active user keys currently tracked in memory.
    #[cfg(test)]
    pub fn active_key_count(&self) -> usize {
        self.state.lock().unwrap().len()
    }
}

#[async_trait]
impl UserActivityRepository for InMemoryUserActivityRepository {
    async fn record_user_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err> {
        // Cap TTL to the hard MAX_RETENTION_DURATION limit
        let effective_ttl = ttl.min(MAX_RETENTION_DURATION);
        let chrono_ttl = chrono::Duration::from_std(effective_ttl)
            .map_err(|e| -> Err { format!("Invalid TTL duration: {e}").into() })?;
        let expires_at = timestamp + chrono_ttl;

        let mut guard = self.state.lock().map_err(|e| -> Err {
            format!("InMemoryUserActivityRepository mutex poisoned: {e}").into()
        })?;

        // Periodic sweep: if >= 5 minutes passed since last sweep, clean all expired user entries
        let should_sweep = match self.last_sweep.lock() {
            Ok(mut last) => {
                let due = match *last {
                    Some(prev) => timestamp - prev >= SWEEP_INTERVAL,
                    None => true,
                };
                if due {
                    *last = Some(timestamp);
                }
                due
            }
            Err(_) => false,
        };

        if should_sweep {
            guard.retain(|_, deque| {
                while let Some(front) = deque.front() {
                    if front.expires_at <= timestamp {
                        deque.pop_front();
                    } else {
                        break;
                    }
                }
                !deque.is_empty()
            });
        }

        let deque = guard.entry((*group_id, *user_id)).or_default();

        // Evict expired records for this specific user
        while let Some(front) = deque.front() {
            if front.expires_at <= timestamp {
                deque.pop_front();
            } else {
                break;
            }
        }

        deque.push_back(MessageRecord {
            timestamp,
            expires_at,
        });

        Ok(())
    }

    async fn count_messages_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        let mut guard = self.state.lock().map_err(|e| -> Err {
            format!("InMemoryUserActivityRepository mutex poisoned: {e}").into()
        })?;

        let key = (*group_id, *user_id);
        let Some(deque) = guard.get_mut(&key) else {
            return Ok(0);
        };

        while let Some(front) = deque.front() {
            if front.expires_at <= now {
                deque.pop_front();
            } else {
                break;
            }
        }

        if deque.is_empty() {
            guard.remove(&key);
            return Ok(0);
        }

        let count = deque
            .iter()
            .filter(|record| record.timestamp >= since)
            .count();

        Ok(count as u32)
    }
}

#[cfg(test)]
mod tests;
