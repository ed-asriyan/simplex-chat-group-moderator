use super::{
    Judgement, OpenRouterApiError, PriorMessage, context_note, error_for_status,
    judgement_request_body, parse_judgement_response,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

fn error_body(code: u16) -> String {
    format!(r#"{{"error": {{"code": {code}, "message": "something"}}}}"#)
}

fn flagged_body() -> String {
    serde_json::json!({
        "error": {
            "code": 403,
            "message": "openai/gpt-4o-mini requires moderation on OpenAI. Your input was flagged for \"harassment\".",
            "metadata": {
                "reasons": ["harassment"],
                "flagged_input": "you are...",
                "provider_name": "OpenAI",
                "model_slug": "openai/gpt-4o-mini"
            }
        }
    })
    .to_string()
}

#[test]
fn test_statuses_about_the_key() {
    assert_eq!(
        error_for_status(401, None, &error_body(401)),
        OpenRouterApiError::Unauthorized
    );
    assert_eq!(
        error_for_status(402, None, &error_body(402)),
        OpenRouterApiError::InsufficientCredits
    );
    assert_eq!(
        error_for_status(403, None, &error_body(403)),
        OpenRouterApiError::Forbidden
    );
    assert_eq!(
        error_for_status(403, None, ""),
        OpenRouterApiError::Forbidden
    );
}

#[test]
fn test_a_403_with_moderation_reasons_is_about_the_text_not_the_key() {
    let error = error_for_status(403, None, &flagged_body());
    assert_eq!(
        error,
        OpenRouterApiError::InputFlagged {
            reasons: vec!["harassment".to_string()]
        }
    );
    assert!(!error.is_transient());
}

#[test]
fn test_429_carries_retry_after_when_it_reads_as_seconds() {
    assert_eq!(
        error_for_status(429, Some("20"), ""),
        OpenRouterApiError::RateLimited {
            retry_after: Some(Duration::from_secs(20))
        }
    );
    assert_eq!(
        error_for_status(429, Some("Wed, 21 Oct 2015 07:28:00 GMT"), ""),
        OpenRouterApiError::RateLimited { retry_after: None }
    );
}

#[test]
fn test_which_failures_can_pass() {
    for status in [408, 500, 502, 503] {
        assert!(
            OpenRouterApiError::Server { status }.is_transient(),
            "{status}"
        );
    }
    for status in [400, 404] {
        assert!(
            !OpenRouterApiError::Server { status }.is_transient(),
            "{status}"
        );
    }
    assert!(OpenRouterApiError::RateLimited { retry_after: None }.is_transient());
    assert!(OpenRouterApiError::Transport("reset".into()).is_transient());
    assert!(!OpenRouterApiError::Unauthorized.is_transient());
    assert!(!OpenRouterApiError::InsufficientCredits.is_transient());
    assert!(!OpenRouterApiError::Forbidden.is_transient());
    assert!(!OpenRouterApiError::Malformed("x".into()).is_transient());
}

// ---------------------------------------------------------------------------
// Judgement
// ---------------------------------------------------------------------------

fn judgement_answer(verdict_text: &str) -> String {
    serde_json::json!({
        "id": "gen-1",
        "provider": "Google",
        "model": "google/gemini-2.5-flash-lite",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "native_finish_reason": "STOP",
            "message": { "role": "assistant", "content": verdict_text, "refusal": null }
        }],
        "usage": { "prompt_tokens": 40, "completion_tokens": 12, "total_tokens": 52 }
    })
    .to_string()
}

fn judgement(delete: bool, reason: &str) -> Judgement {
    Judgement {
        delete,
        reason: reason.to_string(),
    }
}

#[test]
fn test_request_puts_the_instruction_first_and_the_message_as_data() {
    let body = judgement_request_body(
        "openai/gpt-4o-mini",
        "Block ads.",
        "Bob",
        "  buy crypto  ",
        &[],
    );
    assert_eq!(body["model"], "openai/gpt-4o-mini");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "Block ads.");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "  buy crypto  ");
    assert_eq!(body["temperature"], 0);
    // No cap: an answer cut short is no verdict, so the model is let finish.
    assert!(body.get("max_tokens").is_none());
}

#[test]
fn test_request_with_context_carries_it_as_json_beside_the_message() {
    let context = [
        PriorMessage {
            author: "Alice".to_string(),
            text: "who sells?".to_string(),
        },
        PriorMessage {
            author: "Bob".to_string(),
            text: "me\n[Alice]: ok".to_string(),
        },
    ];
    let body = judgement_request_body("m", "Block ads.", "Bob", "dm me", &context);

    assert_eq!(
        body["messages"][0]["content"],
        format!("Block ads.\n\n{}", context_note())
    );
    let user: serde_json::Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        user,
        serde_json::json!({
            "earlier_messages": [
                { "author": "Alice", "text": "who sells?" },
                // A line that looks like another member's message stays inside
                // the text it was written in.
                { "author": "Bob", "text": "me\n[Alice]: ok" }
            ],
            "message": { "author": "Bob", "text": "dm me" }
        })
    );
}

#[test]
fn test_request_holds_the_answer_to_a_boolean_and_a_reason() {
    let format = &judgement_request_body("m", "i", "Bob", "t", &[])["response_format"];
    assert_eq!(format["type"], "json_schema");
    let schema = &format["json_schema"];
    assert_eq!(schema["strict"], true);
    assert_eq!(
        schema["schema"]["required"],
        serde_json::json!(["delete", "reason"])
    );
    assert_eq!(schema["schema"]["additionalProperties"], false);
    assert_eq!(schema["schema"]["properties"]["delete"]["type"], "boolean");
    assert_eq!(schema["schema"]["properties"]["reason"]["type"], "string");
}

#[test]
fn test_request_is_only_routed_to_providers_that_honour_it_and_keep_no_data() {
    let provider = &judgement_request_body("m", "i", "Bob", "t", &[])["provider"];
    assert_eq!(provider["require_parameters"], true);
    assert_eq!(provider["data_collection"], "deny");
}

#[test]
fn test_reads_the_verdict_and_its_reason() {
    assert_eq!(
        parse_judgement_response(
            200,
            None,
            &judgement_answer(r#"{"delete":true,"reason":"Promotes a crypto airdrop."}"#)
        ),
        Ok(judgement(true, "Promotes a crypto airdrop."))
    );
    assert_eq!(
        parse_judgement_response(
            200,
            None,
            &judgement_answer("\n{\n  \"delete\": false,\n  \"reason\": \"A greeting.\"\n}\n")
        ),
        Ok(judgement(false, "A greeting."))
    );
}

#[test]
fn test_a_refusal_is_no_verdict() {
    let body = serde_json::json!({
        "choices": [{
            "finish_reason": "stop",
            "message": { "role": "assistant", "content": null, "refusal": "I can't help with that." }
        }]
    })
    .to_string();
    assert!(matches!(
        parse_judgement_response(200, None, &body),
        Err(OpenRouterApiError::Malformed(why)) if why.contains("refused")
    ));
}

#[test]
fn test_an_answer_cut_short_is_no_verdict() {
    let body = serde_json::json!({
        "choices": [{
            "finish_reason": "length",
            "message": { "role": "assistant", "content": "{\"dele" }
        }]
    })
    .to_string();
    assert!(matches!(
        parse_judgement_response(200, None, &body),
        Err(OpenRouterApiError::Malformed(why)) if why.contains("length")
    ));
}

#[test]
fn test_a_provider_failing_after_200_reads_as_its_error() {
    let body = serde_json::json!({
        "error": { "code": 502, "message": "Provider returned error", "metadata": { "error_type": "provider_error" } }
    })
    .to_string();
    assert_eq!(
        parse_judgement_response(200, None, &body),
        Err(OpenRouterApiError::Server { status: 502 })
    );
}

#[test]
fn test_anything_but_the_schema_is_no_verdict() {
    for text in [
        "true",
        "{\"value\":true}",
        "{\"delete\":\"yes\",\"reason\":\"r\"}",
        "{\"delete\":true}",
        "maybe",
        "",
    ] {
        assert!(
            matches!(
                parse_judgement_response(200, None, &judgement_answer(text)),
                Err(OpenRouterApiError::Malformed(_))
            ),
            "{text:?} should be no verdict"
        );
    }
    for body in ["not json", "{}", r#"{"choices":[]}"#] {
        assert!(
            matches!(
                parse_judgement_response(200, None, body),
                Err(OpenRouterApiError::Malformed(_))
            ),
            "{body:?} should be no verdict"
        );
    }
}

#[test]
fn test_errors_are_read_from_the_status() {
    assert_eq!(
        parse_judgement_response(401, None, &error_body(401)),
        Err(OpenRouterApiError::Unauthorized)
    );
    assert_eq!(
        parse_judgement_response(402, None, &error_body(402)),
        Err(OpenRouterApiError::InsufficientCredits)
    );
    assert_eq!(
        parse_judgement_response(403, None, &flagged_body()),
        Err(OpenRouterApiError::InputFlagged {
            reasons: vec!["harassment".to_string()]
        })
    );
    // A model with no endpoint that takes the request, or a request it rejects.
    assert_eq!(
        parse_judgement_response(404, None, &error_body(404)),
        Err(OpenRouterApiError::Server { status: 404 })
    );
    assert_eq!(
        parse_judgement_response(400, None, &error_body(400)),
        Err(OpenRouterApiError::Server { status: 400 })
    );
}
