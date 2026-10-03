use super::*;

fn some(reason: &str) -> Option<String> {
    Some(reason.to_string())
}

#[test]
fn test_max_words() {
    // Exact boundaries around limit = 3
    assert!(should_moderate_words("one two", 3).is_none());
    assert!(should_moderate_words("one two three", 3).is_none());
    assert_eq!(
        should_moderate_words("one two three four", 3),
        some("4 words")
    );

    // Multiple irregular whitespace characters (spaces, tabs, newlines)
    assert!(should_moderate_words("  one \t\t\n  two \n\n  three  ", 3).is_none());
    assert_eq!(
        should_moderate_words("  one \t\t\n  two \n\n  three \n four ", 3),
        some("4 words")
    );

    // Words with punctuation
    assert!(should_moderate_words("Hello, world! How are you?", 5).is_none());
    assert_eq!(
        should_moderate_words("Hello, world! How are you?", 4),
        some("5 words")
    );

    // Single very long word counts as 1 word
    assert!(should_moderate_words(&"a".repeat(1000), 1).is_none());
}

#[test]
fn test_zero_maximum_never_matches() {
    // Validation rejects 0 on save; a stored 0 must not match every message.
    let large_message = "word ".repeat(100) + "\n".repeat(50).as_str() + &"a".repeat(500);
    assert!(should_moderate_words(&large_message, 0).is_none());
}

#[test]
fn test_rejects_a_zero_maximum() {
    let err = ExceedsMaxWords { max_words: 0 }
        .normalize_and_validate()
        .expect_err("a zero maximum should have been rejected")
        .to_string();
    assert!(err.contains("'Message Exceeds Max Words' needs a maximum of at least 1"));
}
