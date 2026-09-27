use super::{OpenAiModerationGateway, OpenAiModerationGatewayConfig};
use crate::domain::moderator::ports::{
    KeyCheck, OpenAiCategory, OpenAiKeyVerifier, OpenAiModerationClassifier,
};
use crate::infrastructure::drivers::openai_moderation::{
    ModerationApi, ModerationApiError, RawModeration,
};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

// ---------------------------------------------------------------------------
// A scripted Moderation API
// ---------------------------------------------------------------------------

type Answer = Result<RawModeration, ModerationApiError>;

/// Answers from a per-key script (a clean verdict once a key's script runs
/// out), records every call in the order it was made, and — when gated — holds
/// each call until the test hands out a permit.
#[derive(Default)]
struct FakeApi {
    calls: Mutex<Vec<(String, String)>>,
    script: Mutex<HashMap<String, VecDeque<Answer>>>,
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

#[async_trait]
impl ModerationApi for FakeApi {
    async fn moderate(&self, api_key: &str, text: &str) -> Answer {
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
        self.script
            .lock()
            .unwrap()
            .get_mut(api_key)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| Ok(RawModeration::default()))
    }
}

fn config() -> OpenAiModerationGatewayConfig {
    OpenAiModerationGatewayConfig {
        requests_per_minute_per_key: 100,
        max_pending_per_key: 10,
        max_in_flight: 4,
        queue_capacity: 100,
        classify_deadline: Duration::from_secs(60),
        verify_deadline: Duration::from_secs(60),
        rejected_key_ttl: Duration::from_secs(600),
        default_rate_limit_cooldown: Duration::from_secs(20),
    }
}

fn start_gateway(
    api: &Arc<FakeApi>,
    config: OpenAiModerationGatewayConfig,
) -> Arc<OpenAiModerationGateway> {
    Arc::new(OpenAiModerationGateway::new(api.clone(), config))
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_classify_sends_key_and_text_unchanged() {
    let api = FakeApi::new();
    let gateway = start_gateway(&api, config());

    gateway.classify("sk-one", "  привет  ").await.unwrap();

    assert_eq!(
        api.calls(),
        vec![("sk-one".to_string(), "  привет  ".to_string())]
    );
}

#[tokio::test]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
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

    let verdict = gateway.classify("sk-one", "text").await.unwrap();

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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_classify_fails_whenever_openai_gives_no_verdict() {
    let failures = [
        ModerationApiError::Unauthorized,
        ModerationApiError::Forbidden,
        ModerationApiError::InsufficientQuota,
        ModerationApiError::RateLimited { retry_after: None },
        ModerationApiError::Server { status: 503 },
        ModerationApiError::Malformed("x".into()),
        ModerationApiError::Transport("x".into()),
    ];
    for failure in failures {
        let api = FakeApi::new();
        api.answer("sk-one", vec![Err(failure.clone())]);
        let gateway = start_gateway(&api, config());
        assert!(
            gateway.classify("sk-one", "text").await.is_err(),
            "{failure:?} should be an error"
        );
    }
}

// ---------------------------------------------------------------------------
// Key checks
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_verify_reads_openai_answers_about_the_key_without_retrying() {
    let cases = [
        (Ok(RawModeration::default()), KeyCheck::Valid),
        (Err(ModerationApiError::Unauthorized), KeyCheck::Rejected),
        (Err(ModerationApiError::Forbidden), KeyCheck::Forbidden),
        (
            Err(ModerationApiError::InsufficientQuota),
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_verify_retries_a_transient_failure_once() {
    let transient = [
        ModerationApiError::RateLimited {
            retry_after: Some(Duration::from_secs(1)),
        },
        ModerationApiError::Server { status: 503 },
        ModerationApiError::Malformed("x".into()),
        ModerationApiError::Transport("x".into()),
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_verify_is_not_charged_to_the_token_bucket() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            requests_per_minute_per_key: 1,
            ..config()
        },
    );

    gateway.classify("sk-one", "text").await.unwrap();
    assert!(gateway.classify("sk-one", "text").await.is_err());

    // A flooded group must not stop its owner from saving rules.
    assert_eq!(gateway.verify("sk-one").await, KeyCheck::Valid);
    assert_eq!(api.call_count(), 2);
}

// ---------------------------------------------------------------------------
// Pacing per key
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_key_past_its_rate_is_refused_without_asking_openai() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            requests_per_minute_per_key: 2,
            ..config()
        },
    );

    assert!(gateway.classify("sk-one", "a").await.is_ok());
    assert!(gateway.classify("sk-one", "b").await.is_ok());
    assert!(gateway.classify("sk-one", "c").await.is_err());
    assert_eq!(api.call_count(), 2);

    // Another key has a bucket of its own.
    assert!(gateway.classify("sk-two", "d").await.is_ok());
    assert_eq!(api.call_count(), 3);
}

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_key_bucket_refills_over_time() {
    let api = FakeApi::new();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            requests_per_minute_per_key: 2,
            ..config()
        },
    );
    gateway.classify("sk-one", "a").await.unwrap();
    gateway.classify("sk-one", "b").await.unwrap();
    assert!(gateway.classify("sk-one", "c").await.is_err());

    // Two a minute is one every 30 seconds.
    tokio::time::advance(Duration::from_secs(31)).await;

    assert!(gateway.classify("sk-one", "d").await.is_ok());
    assert!(gateway.classify("sk-one", "e").await.is_err());
}

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_rate_limited_key_rests_until_retry_after() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Err(ModerationApiError::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        })],
    );
    let gateway = start_gateway(&api, config());

    assert!(gateway.classify("sk-one", "a").await.is_err());
    assert!(gateway.classify("sk-one", "b").await.is_err());
    assert_eq!(api.call_count(), 1, "a resting key is not sent again");
    assert!(gateway.classify("sk-two", "c").await.is_ok());

    tokio::time::advance(Duration::from_secs(31)).await;

    assert!(gateway.classify("sk-one", "d").await.is_ok());
    assert_eq!(api.keys_called(), vec!["sk-one", "sk-two", "sk-one"]);
}

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_rate_limited_key_without_retry_after_rests_for_the_default() {
    let api = FakeApi::new();
    api.answer(
        "sk-one",
        vec![Err(ModerationApiError::RateLimited { retry_after: None })],
    );
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            default_rate_limit_cooldown: Duration::from_secs(20),
            ..config()
        },
    );

    assert!(gateway.classify("sk-one", "a").await.is_err());
    tokio::time::advance(Duration::from_secs(10)).await;
    assert!(gateway.classify("sk-one", "b").await.is_err());
    tokio::time::advance(Duration::from_secs(11)).await;
    assert!(gateway.classify("sk-one", "c").await.is_ok());
    assert_eq!(api.call_count(), 2);
}

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_key_openai_refused_is_refused_locally_for_a_while() {
    for refusal in [
        ModerationApiError::Unauthorized,
        ModerationApiError::Forbidden,
    ] {
        let api = FakeApi::new();
        api.answer("sk-one", vec![Err(refusal.clone())]);
        let gateway = start_gateway(
            &api,
            OpenAiModerationGatewayConfig {
                rejected_key_ttl: Duration::from_secs(600),
                ..config()
            },
        );

        assert!(gateway.classify("sk-one", "a").await.is_err());
        assert!(gateway.classify("sk-one", "b").await.is_err());
        assert_eq!(api.call_count(), 1, "{refusal:?}: not sent again");
        assert!(gateway.classify("sk-two", "c").await.is_ok());

        tokio::time::advance(Duration::from_secs(601)).await;

        assert!(gateway.classify("sk-one", "d").await.is_ok(), "{refusal:?}");
        assert_eq!(api.call_count(), 3, "{refusal:?}");
    }
}

#[tokio::test(start_paused = true)]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_verify_asks_about_a_refused_key_and_a_valid_answer_lifts_the_refusal() {
    let api = FakeApi::new();
    api.answer("sk-one", vec![Err(ModerationApiError::Forbidden)]);
    let gateway = start_gateway(&api, config());
    assert!(gateway.classify("sk-one", "a").await.is_err());

    // The owner allowed Moderations on the key and sends the link again.
    assert_eq!(gateway.verify("sk-one").await, KeyCheck::Valid);
    assert!(gateway.classify("sk-one", "b").await.is_ok());
    assert_eq!(api.call_count(), 3);
}

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_no_more_than_max_in_flight_requests_run_at_once() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            max_in_flight: 2,
            ..config()
        },
    );

    let jobs: Vec<_> = (0..5)
        .map(|i| {
            let gateway = gateway.clone();
            tokio::spawn(async move { gateway.classify(&format!("sk-{i}"), "text").await })
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_one_key_cannot_take_more_than_its_share_of_the_queue() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            max_in_flight: 1,
            max_pending_per_key: 1,
            ..config()
        },
    );

    let first = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "1").await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    let second = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "2").await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;

    // sk-one already has one job waiting: the next is refused on the spot…
    assert!(gateway.classify("sk-one", "3").await.is_err());
    // …while another key still gets in line.
    let other = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-two", "4").await })
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_key_checks_jump_the_queue() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            max_in_flight: 1,
            ..config()
        },
    );

    let busy = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-busy", "text").await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    let message = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-message", "text").await })
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
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_classify_gives_up_at_its_deadline() {
    let (api, _gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            classify_deadline: Duration::from_millis(50),
            ..config()
        },
    );

    let result = tokio::time::timeout(Duration::from_secs(5), gateway.classify("sk-one", "text"))
        .await
        .expect("classify must not outlive its deadline");
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "red: the OpenAI moderation gateway is not implemented yet"]
async fn test_a_job_that_waited_past_its_deadline_is_never_sent() {
    let (api, gate) = FakeApi::gated();
    let gateway = start_gateway(
        &api,
        OpenAiModerationGatewayConfig {
            max_in_flight: 1,
            classify_deadline: Duration::from_millis(50),
            ..config()
        },
    );

    let first = {
        let gateway = gateway.clone();
        tokio::spawn(async move { gateway.classify("sk-one", "text").await })
    };
    eventually("the first request in flight", || api.call_count() == 1).await;
    assert!(gateway.classify("sk-two", "text").await.is_err());

    gate.add_permits(2);
    let _ = first.await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(api.keys_called(), vec!["sk-one"], "a stale job was sent");
}
