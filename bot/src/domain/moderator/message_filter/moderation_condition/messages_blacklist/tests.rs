use super::should_moderate;

#[test]
fn test_case_sensitive() {
    let blocked = vec!["Hello".to_string(), "World".to_string()];
    assert_eq!(
        should_moderate("Hello", &blocked, true),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("Hello", &blocked, true),
        Some("Hello".to_string())
    );
    assert!(should_moderate("hello", &blocked, true).is_none());
    assert!(should_moderate("Hello World", &blocked, true).is_none());
}

#[test]
fn test_case_insensitive() {
    let blocked = vec!["Hello".to_string(), "World".to_string()];
    assert_eq!(
        should_moderate("Hello", &blocked, false),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("hello", &blocked, false),
        Some("Hello".to_string())
    );
    assert_eq!(
        should_moderate("WORLD", &blocked, false),
        Some("World".to_string())
    );
    assert!(should_moderate("Hello World", &blocked, false).is_none());
    assert!(should_moderate("Hello World", &blocked, false).is_none());
}
