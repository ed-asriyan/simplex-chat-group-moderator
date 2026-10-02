//! The one way the bot talks to remote AI providers: an in-process queue in
//! front of the [`OpenAiApi`] and [`OpenRouterApi`] drivers.
//!
//! Implements the moderator context's `OpenAi` and `OpenRouter` ports. Every
//! request — a message for OpenAI's moderation model, a message for an
//! OpenRouter model following an instruction, or a key to verify — becomes a
//! job on one bounded queue that a single dispatcher drains, so how fast the
//! bot talks to providers is decided in one place, and per key whichever
//! endpoint the key is used for — rate limits are the key's, not the
//! endpoint's:
//! - a token bucket per key (`requests_per_minute_per_key`) keeps a flooded
//!   group from burning its owner's quota — a job with no token is refused,
//!   not delayed;
//! - at most `max_pending_per_key` jobs of one key wait at a time, so one key
//!   cannot fill the queue for everyone else;
//! - at most `max_in_flight` HTTP requests run at once, over all keys;
//! - a key the provider answered 429 for rests until its `Retry-After` has
//!   passed;
//! - a key the provider refused (401/403, no money) is refused locally for
//!   `rejected_key_ttl` instead of being sent again with every message;
//! - key checks jump the queue: an owner is waiting on them.
//!
//! The two drivers fail in their own words; the gateway reads both as one
//! [`Failure`], which is all the pacing needs to know.
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
    ApiRetry, Err, KeyCheck, OpenAi, OpenAiCategory, OpenAiModerationResult, OpenRouter,
    OpenRouterInstructionVerdict,
};
use crate::infrastructure::drivers::openai::{
    HttpOpenAiApi, OpenAiApi, OpenAiApiError, RawModeration,
};
use crate::infrastructure::drivers::openrouter::{
    HttpOpenRouterApi, Judgement, OpenRouterApi, OpenRouterApiError,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct AiGatewayConfig {
    /// Requests one key may make per minute; also the size of its burst.
    requests_per_minute_per_key: u32,
    /// Jobs of one key that may wait for a free slot at the same time.
    max_pending_per_key: usize,
    /// HTTP requests running at the same time, over all keys.
    max_in_flight: usize,
    /// Jobs the queue holds before refusing new ones.
    queue_capacity: usize,
    /// How long a message waits for OpenAI's moderation model, queueing
    /// included.
    classify_deadline: Duration,
    /// How long a message waits for an OpenRouter model following an
    /// instruction, queueing included.
    judge_deadline: Duration,
    /// How long a key check waits, queueing and its one retry included.
    verify_deadline: Duration,
    /// How long a key its provider refused is refused locally.
    rejected_key_ttl: Duration,
    /// How long a rate-limited key rests when the provider sent no
    /// `Retry-After`.
    default_rate_limit_cooldown: Duration,
}

/// Requests one key may make per minute: below the free tier's reported 250.
const REQUESTS_PER_MINUTE_PER_KEY: u32 = 200;

/// HTTP requests running at the same time, over all keys and both providers.
const MAX_IN_FLIGHT: usize = 8;

impl Default for AiGatewayConfig {
    fn default() -> Self {
        Self {
            requests_per_minute_per_key: REQUESTS_PER_MINUTE_PER_KEY,
            max_pending_per_key: 20,
            max_in_flight: MAX_IN_FLIGHT,
            queue_capacity: 1_000,
            classify_deadline: Duration::from_secs(30),
            judge_deadline: Duration::from_secs(30),
            verify_deadline: Duration::from_secs(30),
            rejected_key_ttl: Duration::from_secs(10 * 60),
            default_rate_limit_cooldown: Duration::from_secs(20),
        }
    }
}

/// How long one HTTP attempt may take: as long as the longest deadline, so a
/// slow model gets to finish its answer. A request outliving its caller's
/// deadline still holds a slot until then.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

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

pub struct AiGateway {
    jobs: mpsc::UnboundedSender<Job>,
    keys: Arc<Mutex<KeyStates>>,
    config: AiGatewayConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority {
    /// A key check: an owner is waiting on it.
    Verify,
    /// A message.
    Classify,
}

/// What a job asks, and of which provider.
#[derive(Clone)]
enum Call {
    /// OpenAI's moderation model's scores for a text.
    Moderate { text: String },
    /// An OpenRouter model's yes/no on whether a text is what an instruction
    /// describes.
    Judge {
        model: String,
        instruction: String,
        text: String,
    },
}

/// What the provider answered to a [`Call`], in the driver's terms.
enum Answer {
    Moderation(RawModeration),
    Verdict(Judgement),
}

/// The drivers the dispatcher sends calls to.
struct Apis {
    openai: Arc<dyn OpenAiApi>,
    openrouter: Arc<dyn OpenRouterApi>,
}

impl Call {
    async fn send(&self, apis: &Apis, api_key: &str) -> Result<Answer, Failure> {
        match self {
            Call::Moderate { text } => apis
                .openai
                .moderate(api_key, text)
                .await
                .map(Answer::Moderation)
                .map_err(Failure::from),
            Call::Judge {
                model,
                instruction,
                text,
            } => apis
                .openrouter
                .judge(api_key, model, instruction, text)
                .await
                .map(Answer::Verdict)
                .map_err(Failure::from),
        }
    }
}

/// Why a provider gave no answer, read the same way for both: what the pacing
/// and the key checks need to know, and no more.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
    /// 401: the provider does not know the key.
    Unauthorized,
    /// 403 for the key: it may not make this request.
    Forbidden,
    /// The account behind the key cannot pay.
    NoCredits,
    /// The provider would not look at this text (OpenRouter's moderation in
    /// front of a model flagged it). Not the key's fault.
    TextRefused,
    /// 429: too many requests, with the provider's `Retry-After` if any.
    RateLimited { retry_after: Option<Duration> },
    /// Any other non-success status.
    Server { status: u16, transient: bool },
    /// A success status with a body that is not the answer asked for.
    Malformed(String),
    /// The request never got an HTTP answer.
    Transport(String),
}

impl Failure {
    /// Whether asking again can give a different answer.
    fn is_transient(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Transport(_) => true,
            Self::Server { transient, .. } => *transient,
            Self::Unauthorized
            | Self::Forbidden
            | Self::NoCredits
            | Self::TextRefused
            | Self::Malformed(_) => false,
        }
    }
}

impl From<OpenAiApiError> for Failure {
    fn from(error: OpenAiApiError) -> Self {
        let transient = error.is_transient();
        match error {
            OpenAiApiError::Unauthorized => Self::Unauthorized,
            OpenAiApiError::Forbidden => Self::Forbidden,
            OpenAiApiError::InsufficientQuota => Self::NoCredits,
            OpenAiApiError::RateLimited { retry_after } => Self::RateLimited { retry_after },
            OpenAiApiError::Server { status } => Self::Server { status, transient },
            OpenAiApiError::Malformed(why) => Self::Malformed(why),
            OpenAiApiError::Transport(why) => Self::Transport(why),
        }
    }
}

impl From<OpenRouterApiError> for Failure {
    fn from(error: OpenRouterApiError) -> Self {
        let transient = error.is_transient();
        match error {
            OpenRouterApiError::Unauthorized => Self::Unauthorized,
            OpenRouterApiError::Forbidden => Self::Forbidden,
            OpenRouterApiError::InsufficientCredits => Self::NoCredits,
            OpenRouterApiError::InputFlagged { .. } => Self::TextRefused,
            OpenRouterApiError::RateLimited { retry_after } => Self::RateLimited { retry_after },
            OpenRouterApiError::Server { status } => Self::Server { status, transient },
            OpenRouterApiError::Malformed(why) => Self::Malformed(why),
            OpenRouterApiError::Transport(why) => Self::Transport(why),
        }
    }
}

struct Job {
    api_key: String,
    call: Call,
    priority: Priority,
    deadline: Instant,
    reply: oneshot::Sender<Result<Answer, Failure>>,
}

/// Why a job got no answer from its provider.
#[derive(Debug)]
enum Refusal {
    /// Refused before it was sent: the key is out of tokens, resting, refused,
    /// or has too much waiting already, or the queue is full.
    Local(&'static str),
    /// Waited past its deadline.
    Timeout,
    /// The provider answered with an error.
    Api(Failure),
}

impl AiGateway {
    /// The gateway over HTTPS, with its own drivers and the default pacing.
    /// Starts the dispatcher on the current Tokio runtime.
    pub fn new() -> Result<Self, Err> {
        Ok(Self::start(
            Arc::new(HttpOpenAiApi::new(HTTP_TIMEOUT, HTTP_MAX_ATTEMPTS)?),
            Arc::new(HttpOpenRouterApi::new(HTTP_TIMEOUT)?),
            AiGatewayConfig::default(),
        ))
    }

    /// The gateway over any drivers: what `new` builds on, and what the tests
    /// hand their scripted APIs to.
    fn start(
        openai: Arc<dyn OpenAiApi>,
        openrouter: Arc<dyn OpenRouterApi>,
        config: AiGatewayConfig,
    ) -> Self {
        let (jobs, receiver) = mpsc::unbounded_channel();
        let keys = Arc::new(Mutex::new(KeyStates::default()));
        tokio::spawn(dispatch(
            receiver,
            Arc::new(Apis { openai, openrouter }),
            Arc::new(Semaphore::new(config.max_in_flight.max(1))),
            keys.clone(),
            config.clone(),
        ));
        Self { jobs, keys, config }
    }

    /// Queue one job and wait for its provider's answer, until `deadline` at
    /// most.
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
            return Err(Refusal::Local("the AI queue is not running"));
        }
        match tokio::time::timeout_at(deadline, answer).await {
            Ok(Ok(result)) => result.map_err(Refusal::Api),
            // The dispatcher drops a job that outlived its deadline.
            Ok(Err(_)) | Err(_) => Err(Refusal::Timeout),
        }
    }

    /// A message's call, tried up to `retry.max_attempts` times while the
    /// provider fails in a way that can pass, `retry.retry_delay_seconds` apart. Each
    /// try is admitted against the key's pacing and has the call's whole
    /// deadline. A refusal is logged here, so the ports can just say "no
    /// verdict".
    async fn ask(&self, api_key: &str, call: Call, retry: &ApiRetry) -> Result<Answer, Err> {
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
                        debug!("AI request skipped for key {}: {why}", hint(api_key))
                    }
                    other => warn!(
                        "AI request failed for key {} after {attempt} of {attempts} attempt(s), giving up: {other:?}",
                        hint(api_key)
                    ),
                }
                return Err(format!("no verdict: {refusal:?}").into());
            }
            warn!(
                "AI request failed for key {} on attempt {attempt} of {attempts}, retrying in {}s: {refusal:?}",
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

    /// A key check. It asks the provider even when the key is being refused
    /// locally
    /// — the owner may just have fixed its permissions — and a `Valid` answer
    /// lifts that refusal. It is not charged to the key's token bucket, and a
    /// transient failure is retried once before it reads as `Unreachable`.
    /// `about_model` turns a 400 or 404 into `ModelUnavailable`: for a model
    /// check that is the model, not the provider, saying no.
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
                Refusal::Api(Failure::Unauthorized) => return KeyCheck::Rejected,
                Refusal::Api(Failure::Forbidden) => return KeyCheck::Forbidden,
                Refusal::Api(Failure::NoCredits) => return KeyCheck::QuotaExceeded,
                Refusal::Api(Failure::Server {
                    status: 400 | 404, ..
                }) if about_model => {
                    return KeyCheck::ModelUnavailable;
                }
                Refusal::Api(Failure::RateLimited { retry_after }) => retry_after
                    .unwrap_or(Duration::from_secs(1))
                    .min(Duration::from_secs(2)),
                Refusal::Api(_) => Duration::from_millis(500),
                Refusal::Local(_) | Refusal::Timeout => break,
            };
            warn!(
                "Key check for {} failed on attempt {}: {refusal:?}",
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
impl OpenAi for AiGateway {
    async fn classify(
        &self,
        api_key: &str,
        text: &str,
        retry: &ApiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        let call = Call::Moderate {
            text: text.to_string(),
        };
        match self.ask(api_key, call, retry).await? {
            Answer::Moderation(raw) => Ok(translate(raw)),
            Answer::Verdict(_) => Err("OpenAI answered a moderation call with a verdict".into()),
        }
    }

    async fn verify(&self, api_key: &str) -> KeyCheck {
        let call = || Call::Moderate {
            text: KEY_CHECK_TEXT.to_string(),
        };
        self.check(api_key, call, false).await
    }
}

#[async_trait]
impl OpenRouter for AiGateway {
    async fn matches_instruction(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
        retry: &ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err> {
        let call = Call::Judge {
            model: model.to_string(),
            instruction: instruction.to_string(),
            text: text.to_string(),
        };
        match self.ask(api_key, call, retry).await? {
            Answer::Verdict(judgement) => Ok(OpenRouterInstructionVerdict {
                matches: judgement.delete,
                reason: judgement.reason,
            }),
            Answer::Moderation(_) => Err("OpenRouter answered a model call with moderation".into()),
        }
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
    /// The provider answered 429: leave the key alone until then.
    resting_until: Option<Instant>,
    /// The provider refused the key: refuse it here until then.
    refused_until: Option<Instant>,
}

impl KeyStates {
    /// Let one message of `api_key` into the queue, or say why not. On `Ok` a
    /// token is spent and the message is counted as queued.
    fn admit(
        &mut self,
        api_key: &str,
        now: Instant,
        config: &AiGatewayConfig,
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
            return Err(Refusal::Local("the provider refused this key recently"));
        }
        if state.resting_until.is_some_and(|until| now < until) {
            return Err(Refusal::Local(
                "the provider rate limited this key recently",
            ));
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
            return Err(Refusal::Local("the AI queue is full"));
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

    /// Learn from the provider's answer about `api_key`.
    fn record(
        &mut self,
        api_key: &str,
        error: Option<&Failure>,
        now: Instant,
        config: &AiGatewayConfig,
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
            Some(Failure::Unauthorized | Failure::Forbidden | Failure::NoCredits) => {
                state.refused_until = Some(now + config.rejected_key_ttl)
            }
            Some(Failure::RateLimited { retry_after }) => {
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

/// Takes jobs off the channel and sends them to their provider,
/// `max_in_flight` at a time, key checks first, each queue in arrival order.
async fn dispatch(
    mut receiver: mpsc::UnboundedReceiver<Job>,
    apis: Arc<Apis>,
    slots: Arc<Semaphore>,
    keys: Arc<Mutex<KeyStates>>,
    config: AiGatewayConfig,
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
        let apis = apis.clone();
        let keys = keys.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let result = job.call.send(&apis, &job.api_key).await;
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
