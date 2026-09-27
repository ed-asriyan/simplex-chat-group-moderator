use super::{
    Judgement, MODERATION_MODEL, OpenAiApiError, error_for_status, judgement_request_body,
    moderation_request_body, parse_judgement_response, parse_moderation_response, with_retries,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Errors and retries
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Moderation
// ---------------------------------------------------------------------------

/// A response in the shape OpenAI documents for `omni-moderation-latest`,
/// including the field the driver has no use for.
const FLAGGED_RESPONSE: &str = r#"{
  "id": "modr-0000",
  "model": "omni-moderation-latest",
  "results": [
    {
      "flagged": true,
      "categories": {
        "harassment": false, "harassment/threatening": false,
        "hate": true, "hate/threatening": false,
        "illicit": false, "illicit/violent": false,
        "self-harm": false, "self-harm/intent": false, "self-harm/instructions": false,
        "sexual": false, "sexual/minors": false,
        "violence": true, "violence/graphic": false
      },
      "category_scores": {
        "harassment": 0.01, "harassment/threatening": 0.002,
        "hate": 0.91, "hate/threatening": 0.03,
        "illicit": 0.0001, "illicit/violent": 0.00002,
        "self-harm": 0.0003, "self-harm/intent": 0.0001, "self-harm/instructions": 0.0001,
        "sexual": 0.004, "sexual/minors": 0.0005,
        "violence": 0.72, "violence/graphic": 0.01
      },
      "category_applied_input_types": {
        "hate": ["text"], "violence": ["text"]
      }
    }
  ]
}"#;

#[test]
fn test_request_body_names_the_model_and_carries_the_text_as_is() {
    let text = "  привет,\nмир  ";
    assert_eq!(
        moderation_request_body(text),
        serde_json::json!({ "model": MODERATION_MODEL, "input": text })
    );
}

#[test]
fn test_parses_a_moderation_result_with_openai_category_names() {
    let raw = parse_moderation_response(200, None, FLAGGED_RESPONSE).unwrap();
    assert!(raw.flagged);
    assert_eq!(raw.categories.len(), 13);
    assert_eq!(raw.category_scores.len(), 13);
    assert_eq!(raw.categories.get("hate"), Some(&true));
    assert_eq!(raw.categories.get("hate/threatening"), Some(&false));
    assert_eq!(raw.categories.get("self-harm/instructions"), Some(&false));
    assert_eq!(raw.category_scores.get("hate"), Some(&0.91));
    assert_eq!(raw.category_scores.get("violence"), Some(&0.72));
}

#[test]
fn test_reads_the_first_result() {
    // One text goes in, so one result comes out; anything after it is ignored.
    let body = r#"{"results": [
        {"flagged": false, "categories": {"hate": false}, "category_scores": {"hate": 0.1}},
        {"flagged": true, "categories": {"hate": true}, "category_scores": {"hate": 0.9}}
    ]}"#;
    let raw = parse_moderation_response(200, None, body).unwrap();
    assert!(!raw.flagged);
    assert_eq!(raw.category_scores.get("hate"), Some(&0.1));
}

#[test]
fn test_a_success_without_a_result_is_malformed() {
    for body in [r#"{"results": []}"#, r#"{"id": "x"}"#, "not json", ""] {
        assert!(
            matches!(
                parse_moderation_response(200, None, body),
                Err(OpenAiApiError::Malformed(_))
            ),
            "{body:?} should be malformed"
        );
    }
}

#[test]
fn test_401_and_403_are_about_the_key() {
    assert_eq!(
        parse_moderation_response(401, None, &error_body("invalid_request_error")),
        Err(OpenAiApiError::Unauthorized)
    );
    assert_eq!(
        parse_moderation_response(403, None, &error_body("invalid_request_error")),
        Err(OpenAiApiError::Forbidden)
    );
}

#[test]
fn test_moderation_429_insufficient_quota_is_not_a_rate_limit() {
    assert_eq!(
        parse_moderation_response(429, Some("20"), &error_body("insufficient_quota")),
        Err(OpenAiApiError::InsufficientQuota)
    );
    // Some answers carry it only in `type`.
    let type_only = r#"{"error": {"message": "x", "type": "insufficient_quota", "code": null}}"#;
    assert_eq!(
        parse_moderation_response(429, None, type_only),
        Err(OpenAiApiError::InsufficientQuota)
    );
}

#[test]
fn test_429_rate_limit_carries_retry_after_seconds() {
    assert_eq!(
        parse_moderation_response(429, Some("20"), &error_body("rate_limit_exceeded")),
        Err(OpenAiApiError::RateLimited {
            retry_after: Some(Duration::from_secs(20))
        })
    );
    assert_eq!(
        parse_moderation_response(429, None, &error_body("rate_limit_exceeded")),
        Err(OpenAiApiError::RateLimited { retry_after: None })
    );
    // An HTTP-date or garbage is not a number of seconds; better no hint than a wrong one.
    assert_eq!(
        parse_moderation_response(429, Some("Wed, 21 Oct 2026 07:28:00 GMT"), "not json"),
        Err(OpenAiApiError::RateLimited { retry_after: None })
    );
}

#[test]
fn test_other_statuses_are_server_errors() {
    for status in [400, 404, 500, 502, 503] {
        assert_eq!(
            parse_moderation_response(status, None, &error_body("server_error")),
            Err(OpenAiApiError::Server { status }),
            "status {status}"
        );
    }
}

// ---------------------------------------------------------------------------
// Judgement
// ---------------------------------------------------------------------------

fn judgement_answer(verdict_text: &str) -> String {
    serde_json::json!({
        "id": "resp_1",
        "object": "response",
        "status": "completed",
        "model": "gpt-4o-mini-2024-07-18",
        "output": [{
            "id": "msg_1",
            "type": "message",
            "status": "completed",
            "role": "assistant",
            "content": [{ "type": "output_text", "annotations": [], "text": verdict_text }]
        }]
    })
    .to_string()
}

fn judgement(matches: bool, reason: &str) -> Judgement {
    Judgement {
        matches,
        reason: reason.to_string(),
    }
}

#[test]
fn test_request_puts_the_instruction_first_and_the_message_as_data() {
    let body = judgement_request_body("gpt-4o-mini", "Block ads.", "  buy crypto  ");
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["input"][0]["role"], "system");
    assert_eq!(body["input"][0]["content"][0]["text"], "Block ads.");
    assert_eq!(body["input"][1]["role"], "user");
    assert_eq!(body["input"][1]["content"][0]["type"], "input_text");
    assert_eq!(body["input"][1]["content"][0]["text"], "  buy crypto  ");
    assert_eq!(body["temperature"], 0);
    assert_eq!(body["max_output_tokens"], 200);
    assert_eq!(body["store"], false);
}

#[test]
fn test_request_holds_the_answer_to_a_boolean_and_a_reason() {
    let format = &judgement_request_body("gpt-4o-mini", "i", "t")["text"]["format"];
    assert_eq!(format["type"], "json_schema");
    assert_eq!(format["strict"], true);
    assert_eq!(
        format["schema"]["required"],
        serde_json::json!(["matches", "reason"])
    );
    assert_eq!(format["schema"]["additionalProperties"], false);
    assert_eq!(format["schema"]["properties"]["matches"]["type"], "boolean");
    assert_eq!(format["schema"]["properties"]["reason"]["type"], "string");
}

#[test]
fn test_reads_the_verdict_and_its_reason() {
    assert_eq!(
        parse_judgement_response(
            200,
            None,
            &judgement_answer(r#"{"matches":true,"reason":"Promotes a crypto airdrop."}"#)
        ),
        Ok(judgement(true, "Promotes a crypto airdrop."))
    );
    assert_eq!(
        parse_judgement_response(
            200,
            None,
            &judgement_answer("{\n  \"matches\": false,\n  \"reason\": \"A greeting.\"\n}")
        ),
        Ok(judgement(false, "A greeting."))
    );
}

#[test]
fn test_finds_the_verdict_after_other_output_items() {
    let body = serde_json::json!({
        "status": "completed",
        "output": [
            { "type": "reasoning", "id": "rs_1", "summary": [] },
            { "type": "message", "content": [
                { "type": "output_text", "text": "{\"matches\":true,\"reason\":\"Spam.\"}" }
            ] }
        ]
    })
    .to_string();
    assert_eq!(
        parse_judgement_response(200, None, &body),
        Ok(judgement(true, "Spam."))
    );
}

#[test]
fn test_a_refusal_is_no_verdict() {
    let body = serde_json::json!({
        "status": "completed",
        "output": [{ "type": "message", "content": [{ "type": "refusal", "refusal": "I can't help with that." }] }]
    })
    .to_string();
    assert!(matches!(
        parse_judgement_response(200, None, &body),
        Err(OpenAiApiError::Malformed(why)) if why.contains("refused")
    ));
}

#[test]
fn test_an_answer_cut_short_is_no_verdict() {
    let body = serde_json::json!({
        "status": "incomplete",
        "incomplete_details": { "reason": "max_output_tokens" },
        "output": [{ "type": "message", "content": [{ "type": "output_text", "text": "{\"matc" }] }]
    })
    .to_string();
    assert!(matches!(
        parse_judgement_response(200, None, &body),
        Err(OpenAiApiError::Malformed(_))
    ));
}

#[test]
fn test_anything_but_the_schema_is_no_verdict() {
    for text in [
        "true",
        "{\"value\":true}",
        "{\"matches\":\"yes\",\"reason\":\"r\"}",
        "{\"matches\":true}",
        "maybe",
        "",
    ] {
        assert!(
            matches!(
                parse_judgement_response(200, None, &judgement_answer(text)),
                Err(OpenAiApiError::Malformed(_))
            ),
            "{text:?} should be no verdict"
        );
    }
    for body in ["not json", "{}", r#"{"status":"completed","output":[]}"#] {
        assert!(
            matches!(
                parse_judgement_response(200, None, body),
                Err(OpenAiApiError::Malformed(_))
            ),
            "{body:?} should be no verdict"
        );
    }
}

#[test]
fn test_errors_read_like_every_other_openai_endpoint() {
    let quota = r#"{"error": {"message": "x", "type": "insufficient_quota", "code": "insufficient_quota"}}"#;
    assert_eq!(
        parse_judgement_response(401, None, ""),
        Err(OpenAiApiError::Unauthorized)
    );
    assert_eq!(
        parse_judgement_response(403, None, ""),
        Err(OpenAiApiError::Forbidden)
    );
    assert_eq!(
        parse_judgement_response(429, None, quota),
        Err(OpenAiApiError::InsufficientQuota)
    );
    // A model the key may not use, or one that rejects the request's settings.
    assert_eq!(
        parse_judgement_response(404, None, ""),
        Err(OpenAiApiError::Server { status: 404 })
    );
    assert_eq!(
        parse_judgement_response(400, None, ""),
        Err(OpenAiApiError::Server { status: 400 })
    );
}
