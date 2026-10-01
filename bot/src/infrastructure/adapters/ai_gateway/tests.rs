use super::{AiGateway, AiGatewayConfig};
use crate::domain::moderator::ports::{ApiRetry, KeyCheck, OpenAi, OpenAiCategory, OpenRouter};
use crate::infrastructure::drivers::openai::{OpenAiApi, OpenAiApiError, RawModeration};
use crate::infrastructure::drivers::openrouter::{Judgement, OpenRouterApi, OpenRouterApiError};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

// ---------------------------------------------------------------------------
// Scripted OpenAI and OpenRouter APIs
// ---------------------------------------------------------------------------

type Answer = Result<RawModeration, OpenAiApiError>;

/// Answers from a per-key script (a clean verdict once a key's script runs
/// out), records every call in the order it was made, and — when gated — holds
/// each call until the test hands out a permit.
#[derive(Default)]
struct FakeApi {
    calls: Mutex<Vec<(String, String)>>,
    script: Mutex<HashMap<String, VecDeque<Answer>>>,
    verdicts: Mutex<HashMap<String, VecDeque<Result<Judgement, OpenRouterApiError>>>>,
    judged: Mutex<Vec<(String, String, String)>>,
    gate: Option<Arc<Semaphore>>,
    in_flight: AtomicUsize,
    peak_in_flight: AtomicUsize,
}

impl FakeApi {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every call waits for a permit from the returned semaphore.
    fn gated() -> (Arc<Self>, Arc<Semaphore>) {
        let gate = Arc::new(Semaphore::new(0));
        let api = Arc::new(Self {
            gate: Some(gate.clone()),
            ..Self::default()
        });
        (api, gate)
    }

    fn answer(&self, key: &str, answers: Vec<Answer>) {
        self.script
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_default()
            .extend(answers);
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }

    fn keys_called(&self) -> Vec<String> {
        self.calls().into_iter().map(|(key, _)| key).collect()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl FakeApi {
    /// Scripts the verdicts a key gets from OpenRouter (`false` once the
    /// script runs out). A verdict's reason is "reason: <verdict>".
    fn judge_as(&self, key: &str, verdicts: Vec<Result<bool, OpenRouterApiError>>) {
        self.verdicts
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_default()
            .extend(verdicts.into_iter().map(|verdict| {
                verdict.map(|matches| Judgement {
                    delete: matches,
                    reason: format!("reason: {matches}"),
                })
            }));
    }

    fn judged(&self) -> Vec<(String, String, String)> {
        self.judged.lock().unwrap().clone()
    }

    /// Records the call and holds it at the gate: both providers count
    /// against the same in-flight limit.
    async fn enter(&self, api_key: &str, text: &str) {
        self.calls
            .lock()
            .unwrap()
            .push((api_key.to_string(), text.to_string()));
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.acquire().await.unwrap().forget();
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl OpenAiApi for FakeApi {
    async fn moderate(&self, api_key: &str, text: &str) -> Answer {
        self.enter(api_key, text).await;
        self.script
            .lock()
            .unwrap()
            .get_mut(api_key)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| Ok(RawModeration::default()))
    }
}

#[async_trait]
impl OpenRouterApi for FakeApi {
    async fn judge(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        text: &str,
    ) -> Result<Judgement, OpenRouterApiError> {
        self.enter(api_key, text).await;
        self.judged.lock().unwrap().push((
            api_key.to_string(),
            model.to_string(),
            instruction.to_string(),
        ));
        self.verdicts
            .lock()
            .unwrap()
            .get_mut(api_key)
            .and_then(VecDeque::pop_front)
            .unwrap_or(Ok(Judgement {
                delete: false,
                reason: String::new(),
            }))
    }
}

fn config() -> AiGatewayConfig {
    AiGatewayConfig {
        requests_per_minute_per_key: 100,
        max_pending_per_key: 10,
        max_in_flight: 4,
        queue_capacity: 100,
        classify_deadline: Duration::from_secs(60),
        judge_deadline: Duration::from_secs(60),
        verify_deadline: Duration::from_secs(60),
        rejected_key_ttl: Duration::from_secs(600),
        default_rate_limit_cooldown: Duration::from_secs(20),
    }
}

fn start_gateway(api: &Arc<FakeApi>, config: AiGatewayConfig) -> Arc<AiGateway> {
    Arc::new(AiGateway::start(api.clone(), api.clone(), config))
}

/// Yields to the dispatcher until `condition` holds, or fails the test.
async fn eventually(what: &str, condition: impl Fn() -> bool) {
    for _ in 0..1_000 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("timed out waiting for: {what}");
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_classify_sends_key_and_text_unchanged() {
    let api = FakeApi::new();
    let gateway = start_gateway(&api, config());

    gateway
        .classify("sk-one", "  привет  ", &ApiRetry::NONE)
        .await
        .unwrap();

    assert_eq!(
        api.calls(),
        vec![("sk-one".to_string(), "  привет  ".to_string())]
    );
}

#[tokio::test]
async fn test_classify_translates_openai_names_and_ignores_unknown_categories() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Ok(RawModeration {
            flagged: true,
            categories: HashMap::from([
                ("hate".to_string(), true),
                ("hate/threatening".to_string(), false),
                ("brand-new/category".to_string(), true),
            ]),
            category_scores: HashMap::from([
                ("hate".to_string(), 0.91),
                ("self-harm/intent".to_string(), 0.2),
                ("brand-new/category".to_string(), 0.99),
            ]),
        })],
    );
    let gateway = start_gateway(&api, config());

    let verdict = gateway
        .classify("sk-one", "text", &ApiRetry::NONE)
        .await
        .unwrap();

    assert_eq!(
        verdict.flagged.into_iter().collect::<Vec<_>>(),
        vec![OpenAiCategory::Hate]
    );
    assert_eq!(
        verdict.scores.into_iter().collect::<Vec<_>>(),
        vec![
            (OpenAiCategory::Hate, 0.91),
            (OpenAiCategory::SelfHarmIntent, 0.2)
        ]
    );
}

#[tokio::test]
async fn test_classify_fails_whenever_openai_gives_no_verdict() {
    let failures = [
        OpenAiApiError::Unauthorized,
        OpenAiApiError::Forbidden,
        OpenAiApiError::InsufficientQuota,
        OpenAiApiError::RateLimited { retry_after: None },
        OpenAiApiError::Server { status: 503 },
        OpenAiApiError::Malformed("x".into()),
        OpenAiApiError::Transport("x".into()),
    ];
    for failure in failures {
        let api = FakeApi::new();
        api.answer("sk-one", vec![Err(failure.clone())]);
        let gateway = start_gateway(&api, config());
        assert!(
            gateway
                .classify("sk-one", "text", &ApiRetry::NONE)
                .await
                .is_err(),
            "{failure:?} should be an error"
        );
    }
}

// ---------------------------------------------------------------------------
// Key checks
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_verify_reads_openai_answers_about_the_key_without_retrying() {
    let cases = [
        (Ok(RawModeration::default()), KeyCheck::Valid),
        (Err(OpenAiApiError::Unauthorized), KeyCheck::Rejected),
        (Err(OpenAiApiError::Forbidden), KeyCheck::Forbidden),
        (
            Err(OpenAiApiError::InsufficientQuota),
            KeyCheck::QuotaExceeded,
        ),
    ];
    for (answer, expected) in cases {
        let api = FakeApi::new();
        api.answer("sk-one", vec![answer]);
        let gateway = start_gateway(&api, config());
        assert_eq!(gateway.verify("sk-one").await, expected);
        assert_eq!(api.call_count(), 1, "a definite answer is not retried");
    }
}

#[tokio::test(start_paused = true)]
async fn test_verify_retries_a_transient_failure_once() {
    let transient = [
        OpenAiApiError::RateLimited {
            retry_after: Some(Duration::from_secs(1)),
        },
        OpenAiApiError::Server { status: 503 },
        OpenAiApiError::Malformed("x".into()),
        OpenAiApiError::Transport("x".into()),
    ];
    for failure in transient {
        let api = FakeApi::new();
        api.answer(
            "sk-one",
            vec![Err(failure.clone()), Ok(RawModeration::default())],
        );
        let gateway = start_gateway(&api, config());
        assert_eq!(
            gateway.verify("sk-one").await,
            KeyCheck::Valid,
            "{failure:?}"
        );
        assert_eq!(api.call_count(), 2, "{failure:?}");

        let api = FakeApi::new();
        api.answer("sk-one", vec![Err(failure.clone()), Err(failure.clone())]);
        let gateway = start_gateway(&api, config());
        assert_eq!(
            gateway.verify("sk-one").await,
            KeyCheck::Unreachable,
            "{failure:?}"
        );
        assert_eq!(api.call_count(), 2, "only one retry for {failure:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn test_verify_is_not_charged_to_the_token_bucket() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            requests_per_minute_per_key: 1,
            ..config()
        },
    );

    gateway
        .classify("sk-one", "text", &ApiRetry::NONE)
        .await
        .unwrap();
    assert!(
        gateway
            .classify("sk-one", "text", &ApiRetry::NONE)
            .await
            .is_err()
    );

    // A flooded group must not stop its owner from saving rules.
    assert_eq!(gateway.verify("sk-one").await, KeyCheck::Valid);
    assert_eq!(api.call_count(), 2);
}

// ---------------------------------------------------------------------------
// Pacing per key
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_a_key_past_its_rate_is_refused_without_asking_openai() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            requests_per_minute_per_key: 2,
            ..config()
        },
    );

    assert!(
        gateway
            .classify("sk-one", "a", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert!(
        gateway
            .classify("sk-one", "b", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert!(
        gateway
            .classify("sk-one", "c", &ApiRetry::NONE)
            .await
            .is_err()
    );
    assert_eq!(api.call_count(), 2);

    // Another key has a bucket of its own.
    assert!(
        gateway
            .classify("sk-two", "d", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert_eq!(api.call_count(), 3);
}

#[tokio::test(start_paused = true)]
async fn test_a_key_bucket_refills_over_time() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            requests_per_minute_per_key: 2,
            ..config()
        },
    );
    gateway
        .classify("sk-one", "a", &ApiRetry::NONE)
        .await
        .unwrap();
    gateway
        .classify("sk-one", "b", &ApiRetry::NONE)
        .await
        .unwrap();
    assert!(
        gateway
            .classify("sk-one", "c", &ApiRetry::NONE)
            .await
            .is_err()
    );

    // Two a minute is one every 30 seconds.
    tokio::time::advance(Duration::from_secs(31)).await;

    assert!(
        gateway
            .classify("sk-one", "d", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert!(
        gateway
            .classify("sk-one", "e", &ApiRetry::NONE)
            .await
            .is_err()
    );
}

#[tokio::test(start_paused = true)]
async fn test_a_rate_limited_key_rests_until_retry_after() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Err(OpenAiApiError::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        })],
    );
    let gateway = start_gateway(&api, config());

    assert!(
        gateway
            .classify("sk-one", "a", &ApiRetry::NONE)
            .await
            .is_err()
    );
    assert!(
        gateway
            .classify("sk-one", "b", &ApiRetry::NONE)
            .await
            .is_err()
    );
    assert_eq!(api.call_count(), 1, "a resting key is not sent again");
    assert!(
        gateway
            .classify("sk-two", "c", &ApiRetry::NONE)
            .await
            .is_ok()
    );

    tokio::time::advance(Duration::from_secs(31)).await;

    assert!(
        gateway
            .classify("sk-one", "d", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert_eq!(api.keys_called(), vec!["sk-one", "sk-two", "sk-one"]);
}

#[tokio::test(start_paused = true)]
async fn test_a_rate_limited_key_without_retry_after_rests_for_the_default() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Err(OpenAiApiError::RateLimited { retry_after: None })],
    );
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            default_rate_limit_cooldown: Duration::from_secs(20),
            ..config()
        },
    );

    assert!(
        gateway
            .classify("sk-one", "a", &ApiRetry::NONE)
            .await
            .is_err()
    );
    tokio::time::advance(Duration::from_secs(10)).await;
    assert!(
        gateway
            .classify("sk-one", "b", &ApiRetry::NONE)
            .await
            .is_err()
    );
    tokio::time::advance(Duration::from_secs(11)).await;
    assert!(
        gateway
            .classify("sk-one", "c", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert_eq!(api.call_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn test_a_key_openai_refused_is_refused_locally_for_a_while() {
    for refusal in [OpenAiApiError::Unauthorized, OpenAiApiError::Forbidden] {
        let api = FakeApi::new();
        api.answer("sk-one", vec![Err(refusal.clone())]);
        let gateway = start_gateway(
            &api,
            AiGatewayConfig {
                rejected_key_ttl: Duration::from_secs(600),
                ..config()
            },
        );

        assert!(
            gateway
                .classify("sk-one", "a", &ApiRetry::NONE)
                .await
                .is_err()
        );
        assert!(
            gateway
                .classify("sk-one", "b", &ApiRetry::NONE)
                .await
                .is_err()
        );
        assert_eq!(api.call_count(), 1, "{refusal:?}: not sent again");
        assert!(
            gateway
                .classify("sk-two", "c", &ApiRetry::NONE)
                .await
                .is_ok()
        );

        tokio::time::advance(Duration::from_secs(601)).await;

        assert!(
            gateway
                .classify("sk-one", "d", &ApiRetry::NONE)
                .await
                .is_ok(),
            "{refusal:?}"
        );
        assert_eq!(api.call_count(), 3, "{refusal:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn test_verify_asks_about_a_refused_key_and_a_valid_answer_lifts_the_refusal() {
    let api = FakeApi::new();
    api.answer("sk-one", vec![Err(OpenAiApiError::Forbidden)]);
    let gateway = start_gateway(&api, config());
    assert!(
        gateway
            .classify("sk-one", "a", &ApiRetry::NONE)
            .await
            .is_err()
    );

    // The owner allowed Moderations on the key and sends the link again.
    assert_eq!(gateway.verify("sk-one").await, KeyCheck::Valid);
    assert!(
        gateway
            .classify("sk-one", "b", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert_eq!(api.call_count(), 3);
}

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_more_than_max_in_flight_requests_run_at_once() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            max_in_flight: 2,
            ..config()
        },
    );

    let jobs: Vec<_> = (0..5)
        .map(|i| {
            let gateway = gateway.clone();
            tokio::spawn(async move {
                gateway
                    .classify(&format!("sk-{i}"), "text", &ApiRetry::NONE)
                    .await
            })
        })
        .collect();
    eventually("two requests in flight", || api.call_count() == 2).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(api.call_count(), 2, "a third request started early");

    gate.add_permits(5);
    for job in jobs {
        assert!(job.await.unwrap().is_ok());
    }
    assert_eq!(api.call_count(), 5);
    assert_eq!(api.peak_in_flight.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_one_key_cannot_take_more_than_its_share_of_the_queue() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            max_in_flight: 1,
            max_pending_per_key: 1,
            ..config()
        },
    );

    let first = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "1", &ApiRetry::NONE).await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    let second = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "2", &ApiRetry::NONE).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;

    // sk-one already has one job waiting: the next is refused on the spot…
    assert!(
        gateway
            .classify("sk-one", "3", &ApiRetry::NONE)
            .await
            .is_err()
    );
    // …while another key still gets in line.
    let other = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-two", "4", &ApiRetry::NONE).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;

    gate.add_permits(3);
    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
    assert!(other.await.unwrap().is_ok());
    let texts: Vec<String> = api.calls().into_iter().map(|(_, text)| text).collect();
    assert_eq!(texts, vec!["1", "2", "4"]);
}

#[tokio::test]
async fn test_key_checks_jump_the_queue() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            max_in_flight: 1,
            ..config()
        },
    );

    let busy = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-busy", "text", &ApiRetry::NONE).await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    let message = {
        let gateway = gateway.clone();
        tokio::spawn(async move {
            gateway
                .classify("sk-message", "text", &ApiRetry::NONE)
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    let check = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.verify("sk-owner").await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;

    gate.add_permits(3);
    assert!(busy.await.unwrap().is_ok());
    assert!(message.await.unwrap().is_ok());
    assert_eq!(check.await.unwrap(), KeyCheck::Valid);
    assert_eq!(api.keys_called(), vec!["sk-busy", "sk-owner", "sk-message"]);
}

#[tokio::test]
async fn test_classify_gives_up_at_its_deadline() {
    let (api, _gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            classify_deadline: Duration::from_millis(50),
            ..config()
        },
    );

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        gateway.classify("sk-one", "text", &ApiRetry::NONE),
    )
    .await
    .expect("classify must not outlive its deadline");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_a_job_that_waited_past_its_deadline_is_never_sent() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            max_in_flight: 1,
            classify_deadline: Duration::from_millis(50),
            ..config()
        },
    );

    let first = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "text", &ApiRetry::NONE).await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    assert!(
        gateway
            .classify("sk-two", "text", &ApiRetry::NONE)
            .await
            .is_err()
    );

    gate.add_permits(2);
    let _ = first.await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(api.keys_called(), vec!["sk-one"], "a stale job was sent");
}

// ---------------------------------------------------------------------------
// OpenRouter models following an instruction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_matches_instruction_sends_model_instruction_and_text_and_reads_the_verdict() {
    let api = FakeApi::new();
    api.judge_as("sk-one", vec![Ok(true), Ok(false)]);
    let gateway = start_gateway(&api, config());

    let first = gateway
        .matches_instruction(
            "sk-one",
            "openai/gpt-4o-mini",
            "Block ads.",
            "  buy  ",
            &ApiRetry::NONE,
        )
        .await
        .unwrap();
    assert!(first.matches);
    assert_eq!(first.reason, "reason: true");
    let second = gateway
        .matches_instruction(
            "sk-one",
            "openai/gpt-4o-mini",
            "Block ads.",
            "hi",
            &ApiRetry::NONE,
        )
        .await
        .unwrap();
    assert!(!second.matches);
    assert_eq!(
        api.judged(),
        vec![
            (
                "sk-one".to_string(),
                "openai/gpt-4o-mini".to_string(),
                "Block ads.".to_string()
            ),
            (
                "sk-one".to_string(),
                "openai/gpt-4o-mini".to_string(),
                "Block ads.".to_string()
            ),
        ]
    );
    assert_eq!(api.calls()[0].1, "  buy  ");
}

#[tokio::test]
async fn test_matches_instruction_fails_whenever_openrouter_gives_no_verdict() {
    for failure in [
        OpenRouterApiError::Unauthorized,
        OpenRouterApiError::InsufficientCredits,
        OpenRouterApiError::InputFlagged {
            reasons: vec!["harassment".into()],
        },
        OpenRouterApiError::Server { status: 404 },
        OpenRouterApiError::Malformed("refused".into()),
        OpenRouterApiError::Transport("x".into()),
    ] {
        let api = FakeApi::new();
        api.judge_as("sk-one", vec![Err(failure.clone())]);
        let gateway = start_gateway(&api, config());
        assert!(
            gateway
                .matches_instruction("sk-one", "openai/gpt-4o-mini", "i", "t", &ApiRetry::NONE)
                .await
                .is_err(),
            "{failure:?} should be an error"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn test_an_openrouter_key_is_paced_like_any_other() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        AiGatewayConfig {
            requests_per_minute_per_key: 1,
            ..config()
        },
    );

    assert!(
        gateway
            .matches_instruction("sk-or-one", "openai/gpt-4o-mini", "i", "a", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert!(
        gateway
            .matches_instruction("sk-or-one", "openai/gpt-4o-mini", "i", "b", &ApiRetry::NONE)
            .await
            .is_err()
    );
    // An OpenAI key is a bucket of its own.
    assert!(
        gateway
            .classify("sk-openai", "c", &ApiRetry::NONE)
            .await
            .is_ok()
    );
    assert_eq!(api.call_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn test_an_openrouter_key_without_credits_is_refused_locally_for_a_while() {
    let api = FakeApi::new();
    api.judge_as("sk-one", vec![Err(OpenRouterApiError::InsufficientCredits)]);
    let gateway = start_gateway(&api, config());

    for text in ["a", "b"] {
        assert!(
            gateway
                .matches_instruction("sk-one", "openai/gpt-4o-mini", "i", text, &ApiRetry::NONE)
                .await
                .is_err()
        );
    }
    assert_eq!(api.call_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn test_a_flagged_text_is_no_verdict_but_leaves_the_key_alone() {
    // OpenRouter's moderation in front of the model refused this message; the
    // next one must still be asked, and the flagged one is not asked again.
    let api = FakeApi::new();
    api.judge_as(
        "sk-one",
        vec![
            Err(OpenRouterApiError::InputFlagged {
                reasons: vec!["hate".into()],
            }),
            Ok(true),
        ],
    );
    let gateway = start_gateway(&api, config());
    let retry = ApiRetry {
        max_attempts: 3,
        retry_delay_seconds: 0,
    };

    assert!(
        gateway
            .matches_instruction("sk-one", "openai/gpt-4o-mini", "i", "a", &retry)
            .await
            .is_err()
    );
    assert_eq!(api.call_count(), 1);
    assert!(
        gateway
            .matches_instruction("sk-one", "openai/gpt-4o-mini", "i", "b", &retry)
            .await
            .unwrap()
            .matches
    );
}

#[tokio::test(start_paused = true)]
async fn test_verify_model_asks_that_model_and_reads_its_answers() {
    let cases = [
        (Ok(false), KeyCheck::Valid),
        (Ok(true), KeyCheck::Valid),
        (Err(OpenRouterApiError::Unauthorized), KeyCheck::Rejected),
        (Err(OpenRouterApiError::Forbidden), KeyCheck::Forbidden),
        (
            Err(OpenRouterApiError::InsufficientCredits),
            KeyCheck::QuotaExceeded,
        ),
        (
            Err(OpenRouterApiError::Server { status: 404 }),
            KeyCheck::ModelUnavailable,
        ),
        (
            Err(OpenRouterApiError::Server { status: 400 }),
            KeyCheck::ModelUnavailable,
        ),
    ];
    for (answer, expected) in cases {
        let api = FakeApi::new();
        api.judge_as("sk-one", vec![answer.clone()]);
        let gateway = start_gateway(&api, config());

        assert_eq!(
            gateway.verify_model("sk-one", "openai/gpt-4.1-mini").await,
            expected,
            "{answer:?}"
        );
        assert_eq!(
            api.call_count(),
            1,
            "{answer:?}: a definite answer is not retried"
        );
        assert_eq!(api.judged()[0].1, "openai/gpt-4.1-mini");
    }
}

#[tokio::test(start_paused = true)]
async fn test_verify_model_retries_a_transient_failure_once() {
    let api = FakeApi::new();
    api.judge_as(
        "sk-one",
        vec![Err(OpenRouterApiError::Server { status: 503 }), Ok(false)],
    );
    let gateway = start_gateway(&api, config());
    assert_eq!(
        gateway.verify_model("sk-one", "openai/gpt-4o-mini").await,
        KeyCheck::Valid
    );
    assert_eq!(api.call_count(), 2);

    let api = FakeApi::new();
    api.judge_as(
        "sk-one",
        vec![
            Err(OpenRouterApiError::Transport("x".into())),
            Err(OpenRouterApiError::Transport("x".into())),
        ],
    );
    let gateway = start_gateway(&api, config());
    assert_eq!(
        gateway.verify_model("sk-one", "openai/gpt-4o-mini").await,
        KeyCheck::Unreachable
    );
}

#[tokio::test(start_paused = true)]
async fn test_a_moderation_key_check_does_not_read_400_as_a_model_problem() {
    // Only a model check has a model to blame.
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![
            Err(OpenAiApiError::Server { status: 400 }),
            Err(OpenAiApiError::Server { status: 400 }),
        ],
    );
    let gateway = start_gateway(&api, config());
    assert_eq!(gateway.verify("sk-one").await, KeyCheck::Unreachable);
}

// ---------------------------------------------------------------------------
// Retries
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_a_transient_failure_is_tried_again_up_to_the_owners_attempts() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![
            Err(OpenAiApiError::Server { status: 503 }),
            Err(OpenAiApiError::Server { status: 503 }),
            Ok(RawModeration::default()),
        ],
    );
    let gateway = start_gateway(&api, config());
    let retry = ApiRetry {
        max_attempts: 3,
        retry_delay_seconds: 2,
    };

    let started = tokio::time::Instant::now();
    gateway.classify("sk-one", "text", &retry).await.unwrap();

    assert_eq!(api.call_count(), 3);
    assert!(started.elapsed() >= Duration::from_secs(4));
}

#[tokio::test(start_paused = true)]
async fn test_the_last_attempt_fails_for_good() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Err(OpenAiApiError::Server { status: 500 }); 5],
    );
    let gateway = start_gateway(&api, config());
    let retry = ApiRetry {
        max_attempts: 2,
        retry_delay_seconds: 0,
    };

    assert!(gateway.classify("sk-one", "text", &retry).await.is_err());
    assert_eq!(api.call_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn test_a_failure_that_cannot_pass_is_not_tried_again() {
    let api = FakeApi::new();
    api.answer("sk-one", vec![Err(OpenAiApiError::Unauthorized)]);
    let gateway = start_gateway(&api, config());
    let retry = ApiRetry {
        max_attempts: 5,
        retry_delay_seconds: 0,
    };

    assert!(gateway.classify("sk-one", "text", &retry).await.is_err());
    assert_eq!(api.call_count(), 1);
}
