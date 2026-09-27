use super::{MODERATION_MODEL, ModerationApiError, parse_response, request_body};
use std::time::Duration;

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

fn error_body(kind: &str) -> String {
    format!(
        r#"{{"error": {{"message": "something", "type": "{kind}", "param": null, "code": "{kind}"}}}}"#
    )
}

#[test]
fn test_request_body_names_the_model_and_carries_the_text_as_is() {
    let text = "  привет,\nмир  ";
    assert_eq!(
        request_body(text),
        serde_json::json!({ "model": MODERATION_MODEL, "input": text })
    );
}

#[test]
fn test_parses_a_moderation_result_with_openai_category_names() {
    let raw = parse_response(200, None, FLAGGED_RESPONSE).unwrap();
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
    let raw = parse_response(200, None, body).unwrap();
    assert!(!raw.flagged);
    assert_eq!(raw.category_scores.get("hate"), Some(&0.1));
}

#[test]
fn test_a_success_without_a_result_is_malformed() {
    for body in [r#"{"results": []}"#, r#"{"id": "x"}"#, "not json", ""] {
        assert!(
            matches!(
                parse_response(200, None, body),
                Err(ModerationApiError::Malformed(_))
            ),
            "{body:?} should be malformed"
        );
    }
}

#[test]
fn test_401_and_403_are_about_the_key() {
    assert_eq!(
        parse_response(401, None, &error_body("invalid_request_error")),
        Err(ModerationApiError::Unauthorized)
    );
    assert_eq!(
        parse_response(403, None, &error_body("invalid_request_error")),
        Err(ModerationApiError::Forbidden)
    );
}

#[test]
fn test_429_insufficient_quota_is_not_a_rate_limit() {
    assert_eq!(
        parse_response(429, Some("20"), &error_body("insufficient_quota")),
        Err(ModerationApiError::InsufficientQuota)
    );
    // Some answers carry it only in `type`.
    let type_only = r#"{"error": {"message": "x", "type": "insufficient_quota", "code": null}}"#;
    assert_eq!(
        parse_response(429, None, type_only),
        Err(ModerationApiError::InsufficientQuota)
    );
}

#[test]
fn test_429_rate_limit_carries_retry_after_seconds() {
    assert_eq!(
        parse_response(429, Some("20"), &error_body("rate_limit_exceeded")),
        Err(ModerationApiError::RateLimited {
            retry_after: Some(Duration::from_secs(20))
        })
    );
    assert_eq!(
        parse_response(429, None, &error_body("rate_limit_exceeded")),
        Err(ModerationApiError::RateLimited { retry_after: None })
    );
    // An HTTP-date or garbage is not a number of seconds; better no hint than a wrong one.
    assert_eq!(
        parse_response(429, Some("Wed, 21 Oct 2026 07:28:00 GMT"), "not json"),
        Err(ModerationApiError::RateLimited { retry_after: None })
    );
}

#[test]
fn test_other_statuses_are_server_errors() {
    for status in [400, 404, 500, 502, 503] {
        assert_eq!(
            parse_response(status, None, &error_body("server_error")),
            Err(ModerationApiError::Server { status }),
            "status {status}"
        );
    }
}
