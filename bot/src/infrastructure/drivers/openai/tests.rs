use super::{OpenAiApiError, error_for_status, with_retries};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn error_body(kind: &str) -> String {
    format!(
        r#"{{"error": {{"message": "something", "type": "{kind}", "param": null, "code": "{kind}"}}}}"#
    )
}

#[test]
fn test_statuses_about_the_key() {
    assert_eq!(
        error_for_status(401, None, ""),
        OpenAiApiError::Unauthorized
    );
    assert_eq!(error_for_status(403, None, ""), OpenAiApiError::Forbidden);
}

#[test]
fn test_429_insufficient_quota_is_not_a_rate_limit() {
    assert_eq!(
        error_for_status(429, Some("20"), &error_body("insufficient_quota")),
        OpenAiApiError::InsufficientQuota
    );
    assert_eq!(
        error_for_status(429, Some("20"), &error_body("rate_limit_exceeded")),
        OpenAiApiError::RateLimited {
            retry_after: Some(Duration::from_secs(20))
        }
    );
    assert_eq!(
        error_for_status(429, Some("Wed, 21 Oct 2026 07:28:00 GMT"), "not json"),
        OpenAiApiError::RateLimited { retry_after: None }
    );
}

#[test]
fn test_only_trouble_that_can_pass_is_transient() {
    for transient in [
        OpenAiApiError::RateLimited { retry_after: None },
        OpenAiApiError::Server { status: 503 },
        OpenAiApiError::Transport("x".into()),
    ] {
        assert!(transient.is_transient(), "{transient:?}");
    }
    for definite in [
        OpenAiApiError::Unauthorized,
        OpenAiApiError::Forbidden,
        OpenAiApiError::InsufficientQuota,
        OpenAiApiError::Server { status: 400 },
        OpenAiApiError::Server { status: 404 },
        OpenAiApiError::Malformed("x".into()),
    ] {
        assert!(!definite.is_transient(), "{definite:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn test_transient_failures_are_retried_up_to_the_limit() {
    let calls = AtomicUsize::new(0);
    let result: Result<(), _> = with_retries(3, |_| async {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(OpenAiApiError::Server { status: 503 })
    })
    .await;
    assert_eq!(result, Err(OpenAiApiError::Server { status: 503 }));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn test_a_retry_that_succeeds_ends_the_loop() {
    let calls = AtomicUsize::new(0);
    let result = with_retries(3, |attempt| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move {
            if attempt == 1 {
                Err(OpenAiApiError::Transport("reset".into()))
            } else {
                Ok(attempt)
            }
        }
    })
    .await;
    assert_eq!(result, Ok(2));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn test_a_definite_answer_is_not_asked_again() {
    // A key OpenAI refused is refused on the next try too; asking again only
    // spends the caller's deadline.
    let calls = AtomicUsize::new(0);
    let result: Result<(), _> = with_retries(3, |_| async {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(OpenAiApiError::Unauthorized)
    })
    .await;
    assert_eq!(result, Err(OpenAiApiError::Unauthorized));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
