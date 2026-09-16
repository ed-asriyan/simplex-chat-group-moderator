//! A per-key sliding-window counter with time-to-live eviction.
//!
//! Keeps, for every key, the amounts added recently, and answers one question:
//! how much of it falls inside a window ending now. Everything built on top —
//! "how many messages did this author send", "how many characters", "how many
//! of their messages were moderated" — is that one question with a different
//! `amount` per record, so the counter itself knows none of those words.
//!
//! It is a driver, not an adapter: it holds no domain type (the key is a
//! generic parameter), spells its errors itself, and is synchronous — the
//! asynchronous boundary belongs to the adapters that wrap it. Retention policy
//! is theirs too: this counter honours whatever `ttl` it is handed.

use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::Duration;

type Err = Box<dyn Error + Send + Sync>;

/// Minimum interval between full sweeps of the key map.
const SWEEP_INTERVAL: chrono::Duration = chrono::Duration::minutes(5);

#[derive(Clone, Debug)]
struct Entry {
    at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    amount: u32,
}

pub struct SlidingWindowCounter<K> {
    state: Mutex<HashMap<K, VecDeque<Entry>>>,
    last_sweep: Mutex<Option<DateTime<Utc>>>,
}

impl<K: Eq + Hash + Clone> Default for SlidingWindowCounter<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash + Clone> SlidingWindowCounter<K> {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            last_sweep: Mutex::new(None),
        }
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, HashMap<K, VecDeque<Entry>>>, Err> {
        self.state
            .lock()
            .map_err(|e| -> Err { format!("SlidingWindowCounter mutex poisoned: {e}").into() })
    }

    /// Adds `amount` to `key`'s window at `at`, to be forgotten after `ttl`.
    ///
    /// Capping `ttl` is the caller's business: a counter that silently kept
    /// less than it was asked to would make an adapter's retention policy
    /// unverifiable from the adapter alone.
    pub fn add(&self, key: K, at: DateTime<Utc>, amount: u32, ttl: Duration) -> Result<(), Err> {
        let chrono_ttl = chrono::Duration::from_std(ttl)
            .map_err(|e| -> Err { format!("Invalid TTL duration: {e}").into() })?;
        let expires_at = at + chrono_ttl;

        let mut guard = self.state()?;

        // Periodic sweep, so keys nobody asks about any more do not accumulate.
        let should_sweep = match self.last_sweep.lock() {
            Ok(mut last) => {
                let due = match *last {
                    Some(prev) => at - prev >= SWEEP_INTERVAL,
                    None => true,
                };
                if due {
                    *last = Some(at);
                }
                due
            }
            Err(_) => false,
        };

        if should_sweep {
            guard.retain(|_, entries| {
                evict_expired(entries, at);
                !entries.is_empty()
            });
        }

        let entries = guard.entry(key).or_default();
        evict_expired(entries, at);
        entries.push_back(Entry {
            at,
            expires_at,
            amount,
        });

        Ok(())
    }

    /// Sum of the amounts added to `key` in `[since, now]`.
    ///
    /// Saturating: a total that overflowed is already past every threshold an
    /// owner can configure, while wrapping around would read as "no activity".
    pub fn total(&self, key: &K, since: DateTime<Utc>, now: DateTime<Utc>) -> Result<u32, Err> {
        let mut guard = self.state()?;

        let Some(entries) = guard.get_mut(key) else {
            return Ok(0);
        };
        evict_expired(entries, now);
        if entries.is_empty() {
            guard.remove(key);
            return Ok(0);
        }

        Ok(entries
            .iter()
            .filter(|entry| entry.at >= since)
            .fold(0u32, |acc, entry| acc.saturating_add(entry.amount)))
    }

    /// Drops everything expired as of `now`, and every key left with nothing.
    /// Returns how many keys were removed.
    pub fn purge_expired(&self, now: DateTime<Utc>) -> Result<usize, Err> {
        let mut guard = self.state()?;
        if let Ok(mut last) = self.last_sweep.lock() {
            *last = Some(now);
        }
        let before = guard.len();
        guard.retain(|_, entries| {
            evict_expired(entries, now);
            !entries.is_empty()
        });
        Ok(before.saturating_sub(guard.len()))
    }

    /// Number of keys currently held.
    pub fn active_key_count(&self) -> Result<usize, Err> {
        Ok(self.state()?.len())
    }
}

/// Entries are pushed in `at` order and expire in that same order, so dropping
/// from the front stops at the first one still alive.
fn evict_expired(entries: &mut VecDeque<Entry>, now: DateTime<Utc>) {
    while let Some(front) = entries.front() {
        if front.expires_at <= now {
            entries.pop_front();
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
