use super::{Judgement, parse_response, request_body};
use crate::infrastructure::drivers::openai::OpenAiApiError;

fn answer(verdict_text: &str) -> String {
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
    let body = request_body("gpt-4o-mini", "Block ads.", "  buy crypto  ");
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
    let format = &request_body("gpt-4o-mini", "i", "t")["text"]["format"];
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
        parse_response(
            200,
            None,
            &answer(r#"{"matches":true,"reason":"Promotes a crypto airdrop."}"#)
        ),
        Ok(judgement(true, "Promotes a crypto airdrop."))
    );
    assert_eq!(
        parse_response(
            200,
            None,
            &answer("{\n  \"matches\": false,\n  \"reason\": \"A greeting.\"\n}")
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
        parse_response(200, None, &body),
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
        parse_response(200, None, &body),
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
        parse_response(200, None, &body),
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
                parse_response(200, None, &answer(text)),
                Err(OpenAiApiError::Malformed(_))
            ),
            "{text:?} should be no verdict"
        );
    }
    for body in ["not json", "{}", r#"{"status":"completed","output":[]}"#] {
        assert!(
            matches!(
                parse_response(200, None, body),
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
        parse_response(401, None, ""),
        Err(OpenAiApiError::Unauthorized)
    );
    assert_eq!(
        parse_response(403, None, ""),
        Err(OpenAiApiError::Forbidden)
    );
    assert_eq!(
        parse_response(429, None, quota),
        Err(OpenAiApiError::InsufficientQuota)
    );
    // A model the key may not use, or one that rejects the request's settings.
    assert_eq!(
        parse_response(404, None, ""),
        Err(OpenAiApiError::Server { status: 404 })
    );
    assert_eq!(
        parse_response(400, None, ""),
        Err(OpenAiApiError::Server { status: 400 })
    );
}
