use super::*;
use super::filter::should_moderate;

#[test]
fn test_case_sensitive() {
    let messages = vec!["Hello".to_string(), "World".to_string()];
    assert_eq!(
        should_moderate("Hello", &messages, true),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("Hello", &messages, true),
        Some("Hello".to_string())
    );
    assert!(should_moderate("hello", &messages, true).is_none());
    assert!(should_moderate("Hello World", &messages, true).is_none());
}

#[test]
fn test_case_insensitive() {
    let messages = vec!["Hello".to_string(), "World".to_string()];
    assert_eq!(
        should_moderate("Hello", &messages, false),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("hello", &messages, false),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("WORLD", &messages, false),
        Some("World".to_string())
    );
    assert!(should_moderate("Hello World", &messages, false).is_none());
    assert!(should_moderate("Hello World", &messages, false).is_none());
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

fn err_of(condition: &mut impl Condition) -> String {
    condition
        .normalize_and_validate()
        .expect_err("condition should have been rejected")
        .to_string()
}

#[test]
fn test_rejects_too_many_messages() {
    let mut condition = MatchesExactMessage {
        messages: (0..10_001).map(|i| format!("msg{i}")).collect(),
        case_sensitive: false,
    };
    assert!(err_of(&mut condition).contains("Too many messages"));
}

#[test]
fn test_rejects_too_long_message() {
    let mut condition = MatchesExactMessage {
        messages: vec!["a".repeat(1001)],
        case_sensitive: false,
    };
    assert!(err_of(&mut condition).contains("Message too long"));
}

#[test]
fn test_message_case_sensitivity_flag_is_preserved() {
    let mut condition = MatchesExactMessage {
        messages: vec!["b".to_string(), "a".to_string()],
        case_sensitive: true,
    };

    condition.normalize_and_validate().unwrap();

    assert_eq!(
        condition,
        MatchesExactMessage {
            messages: vec!["a".to_string(), "b".to_string()],
            case_sensitive: true,
        }
    );
}
