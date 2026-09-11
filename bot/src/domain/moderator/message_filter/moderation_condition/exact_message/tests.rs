use super::should_moderate;

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
