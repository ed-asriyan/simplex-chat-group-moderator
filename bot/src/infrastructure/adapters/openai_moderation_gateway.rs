//! The one way the bot talks to OpenAI's Moderation API: an in-process queue
//! in front of the [`ModerationApi`] driver.
//!
//! Implements both OpenAI ports of the moderator context. Every request, a
//! message to classify or a key to verify, becomes a job on one bounded queue
//! that a single dispatcher drains, so how fast the bot talks to OpenAI is
//! decided in one place:
//! - a token bucket per key (`requests_per_minute_per_key`) keeps a flooded
//!   group from burning its owner's quota — a job with no token is refused,
//!   not delayed;
//! - at most `max_pending_per_key` jobs of one key wait at a time, so one key
//!   cannot fill the queue for everyone else;
//! - at most `max_in_flight` HTTP requests run at once, over all keys;
//! - a key OpenAI answered 429 for rests until its `Retry-After` has passed;
//! - a key OpenAI refused (401/403) is refused locally for `rejected_key_ttl`
//!   instead of being sent again with every message;
//! - key checks jump the queue: an owner is waiting on them.
//!
//! Nothing here blocks the caller on a full queue: a job that cannot be taken
//! is an immediate `Err`, and a job that waits past its deadline is dropped
//! without being sent. The ports stay request/response, so this queue can later
//! move out of the process without the domain noticing.

use async_trait::async_trait;
use log::{debug, warn};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio::time::Instant;

use crate::domain::moderator::ports::{
    Err, KeyCheck, OpenAiCategory, OpenAiKeyVerifier, OpenAiModerationClassifier,
    OpenAiModerationResult,
};
use crate::infrastructure::drivers::openai_moderation::{
    ModerationApi, ModerationApiError, RawModeration,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct OpenAiModerationGatewayConfig {
    /// Requests one key may make per minute; also the size of its burst.
    pub requests_per_minute_per_key: u32,
    /// Jobs of one key that may wait for a free slot at the same time.
    pub max_pending_per_key: usize,
    /// HTTP requests running at the same time, over all keys.
    pub max_in_flight: usize,
    /// Jobs the queue holds before refusing new ones.
    pub queue_capacity: usize,
    /// How long a message waits for its verdict, queueing included.
    pub classify_deadline: Duration,
    /// How long a key check waits, queueing and its one retry included.
    pub verify_deadline: Duration,
    /// How long a key OpenAI refused is refused locally.
    pub rejected_key_ttl: Duration,
    /// How long a rate-limited key rests when OpenAI sent no `Retry-After`.
    pub default_rate_limit_cooldown: Duration,
}

/// Requests one key may make per minute: below the free tier's reported 250.
const REQUESTS_PER_MINUTE_PER_KEY: u32 = 200;

/// HTTP requests to OpenAI running at the same time, over all keys.
const MAX_IN_FLIGHT: usize = 8;

impl Default for OpenAiModerationGatewayConfig {
    fn default() -> Self {
        Self {
            requests_per_minute_per_key: REQUESTS_PER_MINUTE_PER_KEY,
            max_pending_per_key: 20,
            max_in_flight: MAX_IN_FLIGHT,
            queue_capacity: 1_000,
            classify_deadline: Duration::from_secs(2),
            verify_deadline: Duration::from_secs(10),
            rejected_key_ttl: Duration::from_secs(10 * 60),
            default_rate_limit_cooldown: Duration::from_secs(20),
        }
    }
}

/// What a key check sends: harmless, short, and the same every time.
const KEY_CHECK_TEXT: &str = "hello";

/// Past this many keys, keys with nothing going on are forgotten.
const KEY_STATES_SOFT_LIMIT: usize = 1_024;

pub struct OpenAiModerationGateway {
    jobs: mpsc::UnboundedSender<Job>,
    keys: Arc<Mutex<KeyStates>>,
    config: OpenAiModerationGatewayConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority {
    /// A key check: an owner is waiting on it.
    Verify,
    /// A message.
    Classify,
}

struct Job {
    api_key: String,
    text: String,
    priority: Priority,
    deadline: Instant,
    reply: oneshot::Sender<Result<RawModeration, ModerationApiError>>,
}

/// Why a job got no answer from OpenAI.
#[derive(Debug)]
enum Refusal {
    /// Refused before it was sent: the key is out of tokens, resting, refused,
    /// or has too much waiting already, or the queue is full.
    Local(&'static str),
    /// Waited past its deadline.
    Timeout,
    /// OpenAI answered with an error.
    Api(ModerationApiError),
}

impl OpenAiModerationGateway {
    /// Starts the dispatcher on the current Tokio runtime.
    pub fn new(api: Arc<dyn ModerationApi>, config: OpenAiModerationGatewayConfig) -> Self {
        let (jobs, receiver) = mpsc::unbounded_channel();
        let keys = Arc::new(Mutex::new(KeyStates::default()));
        tokio::spawn(dispatch(
            receiver,
            api,
            Arc::new(Semaphore::new(config.max_in_flight.max(1))),
            keys.clone(),
            config.clone(),
        ));
        Self { jobs, keys, config }
    }

    /// Queue one job and wait for OpenAI's answer, until `deadline` at most.
    async fn submit(
        &self,
        api_key: &str,
        text: &str,
        priority: Priority,
        deadline: Instant,
    ) -> Result<RawModeration, Refusal> {
        let (reply, answer) = oneshot::channel();
        let job = Job {
            api_key: api_key.to_string(),
            text: text.to_string(),
            priority,
            deadline,
            reply,
        };
        if self.jobs.send(job).is_err() {
            if priority == Priority::Classify {
                lock(&self.keys).unqueue(api_key);
            }
            return Err(Refusal::Local("the OpenAI queue is not running"));
        }
        match tokio::time::timeout_at(deadline, answer).await {
            Ok(Ok(result)) => result.map_err(Refusal::Api),
            // The dispatcher drops a job that outlived its deadline.
            Ok(Err(_)) | Err(_) => Err(Refusal::Timeout),
        }
    }
}

#[async_trait]
impl OpenAiModerationClassifier for OpenAiModerationGateway {
    async fn classify(&self, api_key: &str, text: &str) -> Result<OpenAiModerationResult, Err> {
        let now = Instant::now();
        let deadline = now + self.config.classify_deadline;
        let admitted = lock(&self.keys).admit(api_key, now, &self.config);
        let result = match admitted {
            Ok(()) => {
                self.submit(api_key, text, Priority::Classify, deadline)
                    .await
            }
            Err(refusal) => Err(refusal),
        };
        match result {
            Ok(raw) => Ok(translate(raw)),
            Err(refusal) => {
                match &refusal {
                    Refusal::Local(why) => {
                        debug!("OpenAI moderation skipped for key {}: {why}", hint(api_key))
                    }
                    other => warn!(
                        "OpenAI moderation failed for key {}: {other:?}",
                        hint(api_key)
                    ),
                }
                Err(format!("no OpenAI verdict: {refusal:?}").into())
            }
        }
    }
}

#[async_trait]
impl OpenAiKeyVerifier for OpenAiModerationGateway {
    /// A key check asks OpenAI even when the key is being refused locally —
    /// the owner may just have fixed its permissions — and a `Valid` answer
    /// lifts that refusal. It is not charged to the key's token bucket, and a
    /// transient failure is retried once before it reads as `Unreachable`.
    async fn verify(&self, api_key: &str) -> KeyCheck {
        let deadline = Instant::now() + self.config.verify_deadline;
        for attempt in 0..2 {
            let refusal = match self
                .submit(api_key, KEY_CHECK_TEXT, Priority::Verify, deadline)
                .await
            {
                Ok(_) => return KeyCheck::Valid,
                Err(refusal) => refusal,
            };
            let backoff = match refusal {
                Refusal::Api(ModerationApiError::Unauthorized) => return KeyCheck::Rejected,
                Refusal::Api(ModerationApiError::Forbidden) => return KeyCheck::Forbidden,
                Refusal::Api(ModerationApiError::InsufficientQuota) => {
                    return KeyCheck::QuotaExceeded;
                }
                Refusal::Api(ModerationApiError::RateLimited { retry_after }) => retry_after
                    .unwrap_or(Duration::from_secs(1))
                    .min(Duration::from_secs(2)),
                Refusal::Api(_) => Duration::from_millis(500),
                Refusal::Local(_) | Refusal::Timeout => break,
            };
            warn!(
                "OpenAI key check for {} failed on attempt {}: {refusal:?}",
                hint(api_key),
                attempt + 1
            );
            if Instant::now() + backoff >= deadline {
                break;
            }
            tokio::time::sleep(backoff).await;
        }
        KeyCheck::Unreachable
    }
}

/// OpenAI's verdict in the moderator's terms. Categories this build does not
/// know are dropped.
fn translate(raw: RawModeration) -> OpenAiModerationResult {
    OpenAiModerationResult {
        flagged: raw
            .categories
            .iter()
            .filter(|(_, flagged)| **flagged)
            .filter_map(|(name, _)| OpenAiCategory::from_api_name(name))
            .collect(),
        scores: raw
            .category_scores
            .iter()
            .filter_map(|(name, score)| Some((OpenAiCategory::from_api_name(name)?, *score)))
            .collect(),
    }
}

/// A key as it may appear in a log: its last four characters.
fn hint(api_key: &str) -> String {
    let mut tail: Vec<char> = api_key.chars().rev().take(4).collect();
    tail.reverse();
    format!("…{}", tail.into_iter().collect::<String>())
}

fn lock(keys: &Mutex<KeyStates>) -> MutexGuard<'_, KeyStates> {
    // The state is counters and deadlines; one a panicking holder left half
    // updated is still better than no moderation at all.
    keys.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// Per-key pacing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct KeyStates {
    keys: HashMap<String, KeyState>,
    /// Messages waiting for a free slot, over all keys.
    queued: usize,
}

struct KeyState {
    /// Token bucket: tokens left, and when they were last topped up.
    tokens: f64,
    refilled_at: Instant,
    /// Messages of this key waiting for a free slot.
    queued: usize,
    /// OpenAI answered 429: leave the key alone until then.
    resting_until: Option<Instant>,
    /// OpenAI refused the key: refuse it here until then.
    refused_until: Option<Instant>,
}

impl KeyStates {
    /// Let one message of `api_key` into the queue, or say why not. On `Ok` a
    /// token is spent and the message is counted as queued.
    fn admit(
        &mut self,
        api_key: &str,
        now: Instant,
        config: &OpenAiModerationGatewayConfig,
    ) -> Result<(), Refusal> {
        if self.keys.len() > KEY_STATES_SOFT_LIMIT {
            self.forget_idle_keys(now);
        }
        let capacity = f64::from(config.requests_per_minute_per_key.max(1));
        let state = self.keys.entry(api_key.to_string()).or_insert(KeyState {
            tokens: capacity,
            refilled_at: now,
            queued: 0,
            resting_until: None,
            refused_until: None,
        });

        if state.refused_until.is_some_and(|until| now < until) {
            return Err(Refusal::Local("OpenAI refused this key recently"));
        }
        if state.resting_until.is_some_and(|until| now < until) {
            return Err(Refusal::Local("OpenAI rate limited this key recently"));
        }
        let elapsed = now
            .saturating_duration_since(state.refilled_at)
            .as_secs_f64();
        state.tokens = (state.tokens + elapsed * capacity / 60.0).min(capacity);
        state.refilled_at = now;
        if state.tokens < 1.0 {
            return Err(Refusal::Local("this key is over its rate"));
        }
        if state.queued >= config.max_pending_per_key {
            return Err(Refusal::Local("this key has too many messages waiting"));
        }
        if self.queued >= config.queue_capacity {
            return Err(Refusal::Local("the OpenAI queue is full"));
        }
        state.tokens -= 1.0;
        state.queued += 1;
        self.queued += 1;
        Ok(())
    }

    /// A queued message of `api_key` left the queue: sent, or dropped.
    fn unqueue(&mut self, api_key: &str) {
        if let Some(state) = self.keys.get_mut(api_key) {
            state.queued = state.queued.saturating_sub(1);
        }
        self.queued = self.queued.saturating_sub(1);
    }

    /// Learn from OpenAI's answer about `api_key`.
    fn record(
        &mut self,
        api_key: &str,
        result: &Result<RawModeration, ModerationApiError>,
        now: Instant,
        config: &OpenAiModerationGatewayConfig,
    ) {
        let Some(state) = self.keys.get_mut(api_key) else {
            // A key check for a key no message has used: nothing to lift, and
            // a refusal is only worth remembering for keys that send messages.
            return;
        };
        match result {
            Ok(_) => {
                state.refused_until = None;
                state.resting_until = None;
            }
            Err(
                ModerationApiError::Unauthorized
                | ModerationApiError::Forbidden
                | ModerationApiError::InsufficientQuota,
            ) => state.refused_until = Some(now + config.rejected_key_ttl),
            Err(ModerationApiError::RateLimited { retry_after }) => {
                state.resting_until =
                    Some(now + retry_after.unwrap_or(config.default_rate_limit_cooldown));
            }
            Err(_) => {}
        }
    }

    /// Drop keys with nothing waiting, nothing to remember and a full bucket:
    /// forgetting them changes nothing.
    fn forget_idle_keys(&mut self, now: Instant) {
        let refill_time = Duration::from_secs(60);
        self.keys.retain(|_, state| {
            state.queued > 0
                || state.refused_until.is_some_and(|until| now < until)
                || state.resting_until.is_some_and(|until| now < until)
                || now.saturating_duration_since(state.refilled_at) < refill_time
        });
    }
}

// ---------------------------------------------------------------------------
// The dispatcher
// ---------------------------------------------------------------------------

/// Takes jobs off the channel and sends them to OpenAI, `max_in_flight` at a
/// time, key checks first, each queue in arrival order.
async fn dispatch(
    mut receiver: mpsc::UnboundedReceiver<Job>,
    api: Arc<dyn ModerationApi>,
    slots: Arc<Semaphore>,
    keys: Arc<Mutex<KeyStates>>,
    config: OpenAiModerationGatewayConfig,
) {
    let mut checks: VecDeque<Job> = VecDeque::new();
    let mut messages: VecDeque<Job> = VecDeque::new();
    let mut open = true;
    let enqueue =
        |job: Job, checks: &mut VecDeque<Job>, messages: &mut VecDeque<Job>| match job.priority {
            Priority::Verify => checks.push_back(job),
            Priority::Classify => messages.push_back(job),
        };

    loop {
        if checks.is_empty() && messages.is_empty() {
            if !open {
                return;
            }
            match receiver.recv().await {
                Some(job) => enqueue(job, &mut checks, &mut messages),
                None => return,
            }
        }
        while let Ok(job) = receiver.try_recv() {
            enqueue(job, &mut checks, &mut messages);
        }

        // Keep taking jobs while waiting for a slot, so a key check that
        // arrives meanwhile still goes before the messages already waiting.
        let slot: OwnedSemaphorePermit = tokio::select! {
            biased;
            slot = slots.clone().acquire_owned() => match slot {
                Ok(slot) => slot,
                Err(_) => return,
            },
            job = receiver.recv(), if open => {
                match job {
                    Some(job) => enqueue(job, &mut checks, &mut messages),
                    None => open = false,
                }
                continue;
            }
        };

        let Some(job) = next_live_job(&mut checks, &mut messages, &keys) else {
            drop(slot);
            continue;
        };
        let api = api.clone();
        let keys = keys.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let result = api.moderate(&job.api_key, &job.text).await;
            lock(&keys).record(&job.api_key, &result, Instant::now(), &config);
            // The caller may have given up already; nobody to tell then.
            let _ = job.reply.send(result);
            drop(slot);
        });
    }
}

/// The next job worth sending: key checks first. A job past its deadline is
/// dropped unsent — its caller has stopped waiting.
fn next_live_job(
    checks: &mut VecDeque<Job>,
    messages: &mut VecDeque<Job>,
    keys: &Mutex<KeyStates>,
) -> Option<Job> {
    let now = Instant::now();
    while let Some(job) = checks.pop_front().or_else(|| messages.pop_front()) {
        if job.priority == Priority::Classify {
            lock(keys).unqueue(&job.api_key);
        }
        if job.deadline > now {
            return Some(job);
        }
    }
    None
}
