//! The one way the bot talks to OpenAI: an in-process queue in front of the
//! [`OpenAiApi`] driver.
//!
//! Implements the moderator context's `OpenAi` port. Every request — a
//! message for the moderation model, a message for a model following an
//! instruction, or a key to verify — becomes a job on one bounded queue
//! that a single dispatcher drains, so how fast the bot talks to OpenAI is
//! decided in one place, and per key whichever endpoint the key is used for —
//! OpenAI's rate limits are the key's, not the endpoint's:
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
    Err, KeyCheck, OpenAi, OpenAiCategory, OpenAiInstructionVerdict, OpenAiModerationResult,
    OpenAiRetry,
};
use crate::infrastructure::drivers::openai::{
    HttpOpenAiApi, Judgement, OpenAiApi, OpenAiApiError, RawModeration,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct OpenAiGatewayConfig {
    /// Requests one key may make per minute; also the size of its burst.
    requests_per_minute_per_key: u32,
    /// Jobs of one key that may wait for a free slot at the same time.
    max_pending_per_key: usize,
    /// HTTP requests running at the same time, over all keys.
    max_in_flight: usize,
    /// Jobs the queue holds before refusing new ones.
    queue_capacity: usize,
    /// How long a message waits for the moderation model, queueing included.
    classify_deadline: Duration,
    /// How long a message waits for a model following an instruction,
    /// queueing included. Longer: a chat model answers slower than the
    /// moderation endpoint.
    judge_deadline: Duration,
    /// How long a key check waits, queueing and its one retry included.
    verify_deadline: Duration,
    /// How long a key OpenAI refused is refused locally.
    rejected_key_ttl: Duration,
    /// How long a rate-limited key rests when OpenAI sent no `Retry-After`.
    default_rate_limit_cooldown: Duration,
}

/// Requests one key may make per minute: below the free tier's reported 250.
const REQUESTS_PER_MINUTE_PER_KEY: u32 = 200;

/// HTTP requests to OpenAI running at the same time, over all keys.
const MAX_IN_FLIGHT: usize = 8;

impl Default for OpenAiGatewayConfig {
    fn default() -> Self {
        Self {
            requests_per_minute_per_key: REQUESTS_PER_MINUTE_PER_KEY,
            max_pending_per_key: 20,
            max_in_flight: MAX_IN_FLIGHT,
            queue_capacity: 1_000,
            classify_deadline: Duration::from_secs(2),
            judge_deadline: Duration::from_secs(4),
            verify_deadline: Duration::from_secs(10),
            rejected_key_ttl: Duration::from_secs(10 * 60),
            default_rate_limit_cooldown: Duration::from_secs(20),
        }
    }
}

/// How long one HTTP attempt may take. A request outliving its caller's
/// deadline still holds a slot, so this stays short: a hung OpenAI costs
/// seconds of capacity, not more.
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Tries per HTTP request. One: how often a message is tried again is the
/// owner's setting on the condition, and the gateway's `ask` does the retrying
/// so that every try is paced against the key like any other request.
const HTTP_MAX_ATTEMPTS: usize = 1;

/// What a key check sends: harmless, short, and the same every time.
const KEY_CHECK_TEXT: &str = "hello";

/// What a model check asks the model to do with it: the shortest instruction
/// that still exercises the whole request, schema included.
const KEY_CHECK_INSTRUCTION: &str = "Answer false.";

/// Past this many keys, keys with nothing going on are forgotten.
const KEY_STATES_SOFT_LIMIT: usize = 1_024;

pub struct OpenAiGateway {
    jobs: mpsc::UnboundedSender<Job>,
    keys: Arc<Mutex<KeyStates>>,
    config: OpenAiGatewayConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority {
    /// A key check: an owner is waiting on it.
    Verify,
    /// A message.
    Classify,
}

/// What a job asks OpenAI.
#[derive(Clone)]
enum Call {
    /// The moderation model's scores for a text.
    Moderate { text: String },
    /// A model's yes/no on whether a text is what an instruction describes.
    Judge {
        model: String,
        instruction: String,
        text: String,
    },
}

/// What OpenAI answered to a [`Call`], in the driver's terms.
enum Answer {
    Moderation(RawModeration),
    Verdict(Judgement),
}

impl Call {
    async fn send(&self, api: &dyn OpenAiApi, api_key: &str) -> Result<Answer, OpenAiApiError> {
        match self {
            Call::Moderate { text } => api.moderate(api_key, text).await.map(Answer::Moderation),
            Call::Judge {
                model,
                instruction,
                text,
            } => api
                .judge(api_key, model, instruction, text)
                .await
                .map(Answer::Verdict),
        }
    }
}

struct Job {
    api_key: String,
    call: Call,
    priority: Priority,
    deadline: Instant,
    reply: oneshot::Sender<Result<Answer, OpenAiApiError>>,
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
    Api(OpenAiApiError),
}

impl OpenAiGateway {
    /// The gateway over HTTPS, with its own driver and the default pacing.
    /// Starts the dispatcher on the current Tokio runtime.
    pub fn new() -> Result<Self, Err> {
        Ok(Self::start(
            Arc::new(HttpOpenAiApi::new(HTTP_TIMEOUT, HTTP_MAX_ATTEMPTS)?),
            OpenAiGatewayConfig::default(),
        ))
    }

    /// The gateway over any driver: what `new` builds on, and what the tests
    /// hand their scripted API to.
    fn start(api: Arc<dyn OpenAiApi>, config: OpenAiGatewayConfig) -> Self {
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
        call: Call,
        priority: Priority,
        deadline: Instant,
    ) -> Result<Answer, Refusal> {
        let (reply, answer) = oneshot::channel();
        let job = Job {
            api_key: api_key.to_string(),
            call,
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

    /// A message's call, tried up to `retry.max_attempts` times while OpenAI
    /// fails in a way that can pass, `retry.retry_delay_seconds` apart. Each
    /// try is admitted against the key's pacing and has the call's whole
    /// deadline. A refusal is logged here, so the ports can just say "no
    /// verdict".
    async fn ask(&self, api_key: &str, call: Call, retry: &OpenAiRetry) -> Result<Answer, Err> {
        let attempts = retry.max_attempts.max(1);
        let delay = Duration::from_secs(u64::from(retry.retry_delay_seconds));
        let mut attempt = 1;
        loop {
            let refusal = match self.ask_once(api_key, &call).await {
                Ok(answer) => return Ok(answer),
                Err(refusal) => refusal,
            };
            let can_pass = match &refusal {
                Refusal::Api(error) => error.is_transient(),
                Refusal::Timeout => true,
                Refusal::Local(_) => false,
            };
            if !can_pass || attempt >= attempts {
                match &refusal {
                    Refusal::Local(why) => {
                        debug!("OpenAI request skipped for key {}: {why}", hint(api_key))
                    }
                    other => warn!(
                        "OpenAI request failed for key {} after {attempt} of {attempts} attempt(s), giving up: {other:?}",
                        hint(api_key)
                    ),
                }
                return Err(format!("no OpenAI verdict: {refusal:?}").into());
            }
            warn!(
                "OpenAI request failed for key {} on attempt {attempt} of {attempts}, retrying in {}s: {refusal:?}",
                hint(api_key),
                delay.as_secs()
            );
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    async fn ask_once(&self, api_key: &str, call: &Call) -> Result<Answer, Refusal> {
        let now = Instant::now();
        let deadline = now
            + match call {
                Call::Moderate { .. } => self.config.classify_deadline,
                Call::Judge { .. } => self.config.judge_deadline,
            };
        lock(&self.keys).admit(api_key, now, &self.config)?;
        self.submit(api_key, call.clone(), Priority::Classify, deadline)
            .await
    }

    /// A key check. It asks OpenAI even when the key is being refused locally
    /// — the owner may just have fixed its permissions — and a `Valid` answer
    /// lifts that refusal. It is not charged to the key's token bucket, and a
    /// transient failure is retried once before it reads as `Unreachable`.
    /// `about_model` turns a 400 or 404 into `ModelUnavailable`: for a model
    /// check that is the model, not OpenAI, saying no.
    async fn check(&self, api_key: &str, call: impl Fn() -> Call, about_model: bool) -> KeyCheck {
        let deadline = Instant::now() + self.config.verify_deadline;
        for attempt in 0..2 {
            let refusal = match self
                .submit(api_key, call(), Priority::Verify, deadline)
                .await
            {
                Ok(_) => return KeyCheck::Valid,
                Err(refusal) => refusal,
            };
            let backoff = match refusal {
                Refusal::Api(OpenAiApiError::Unauthorized) => return KeyCheck::Rejected,
                Refusal::Api(OpenAiApiError::Forbidden) => return KeyCheck::Forbidden,
                Refusal::Api(OpenAiApiError::InsufficientQuota) => {
                    return KeyCheck::QuotaExceeded;
                }
                Refusal::Api(OpenAiApiError::Server { status: 400 | 404 }) if about_model => {
                    return KeyCheck::ModelUnavailable;
                }
                Refusal::Api(OpenAiApiError::RateLimited { retry_after }) => retry_after
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

#[async_trait]
impl OpenAi for OpenAiGateway {
    async fn classify(
        &self,
        api_key: &str,
        text: &str,
        retry: &OpenAiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        let call = Call::Moderate {
            text: text.to_string(),
        };
        match self.ask(api_key, call, retry).await? {
            Answer::Moderation(raw) => Ok(translate(raw)),
            Answer::Verdict(_) => Err("OpenAI answered a moderation call with a verdict".into()),
        }
    }

    async fn matches_instruction(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
        retry: &OpenAiRetry,
    ) -> Result<OpenAiInstructionVerdict, Err> {
        let call = Call::Judge {
            model: model.to_string(),
            instruction: instruction.to_string(),
            text: text.to_string(),
        };
        match self.ask(api_key, call, retry).await? {
            Answer::Verdict(judgement) => Ok(OpenAiInstructionVerdict {
                matches: judgement.delete,
                reason: judgement.reason,
            }),
            Answer::Moderation(_) => Err("OpenAI answered a model call with moderation".into()),
        }
    }

    async fn verify(&self, api_key: &str) -> KeyCheck {
        let call = || Call::Moderate {
            text: KEY_CHECK_TEXT.to_string(),
        };
        self.check(api_key, call, false).await
    }

    async fn verify_model(&self, api_key: &str, model: &str) -> KeyCheck {
        let call = || Call::Judge {
            model: model.to_string(),
            instruction: KEY_CHECK_INSTRUCTION.to_string(),
            text: KEY_CHECK_TEXT.to_string(),
        };
        self.check(api_key, call, true).await
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
        config: &OpenAiGatewayConfig,
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
        error: Option<&OpenAiApiError>,
        now: Instant,
        config: &OpenAiGatewayConfig,
    ) {
        let Some(state) = self.keys.get_mut(api_key) else {
            // A key check for a key no message has used: nothing to lift, and
            // a refusal is only worth remembering for keys that send messages.
            return;
        };
        match error {
            None => {
                state.refused_until = None;
                state.resting_until = None;
            }
            Some(
                OpenAiApiError::Unauthorized
                | OpenAiApiError::Forbidden
                | OpenAiApiError::InsufficientQuota,
            ) => state.refused_until = Some(now + config.rejected_key_ttl),
            Some(OpenAiApiError::RateLimited { retry_after }) => {
                state.resting_until =
                    Some(now + retry_after.unwrap_or(config.default_rate_limit_cooldown));
            }
            Some(_) => {}
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
    api: Arc<dyn OpenAiApi>,
    slots: Arc<Semaphore>,
    keys: Arc<Mutex<KeyStates>>,
    config: OpenAiGatewayConfig,
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
            let result = job.call.send(api.as_ref(), &job.api_key).await;
            lock(&keys).record(&job.api_key, result.as_ref().err(), Instant::now(), &config);
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
