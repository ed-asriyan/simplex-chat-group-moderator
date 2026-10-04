use super::*;

fn some(reason: &str) -> Option<String> {
    Some(reason.to_string())
}

#[test]
fn test_max_characters() {
    // Exact boundaries around limit = 10
    assert!(should_moderate_characters("012345678", 10).is_none());
    assert!(should_moderate_characters("0123456789", 10).is_none());
    assert_eq!(
        should_moderate_characters("0123456789a", 10),
        some("11 characters")
    );

    // Multibyte Unicode characters (e.g. Cyrillic: 6 characters = 12 UTF-8 bytes)
    assert!(should_moderate_characters("Привет", 6).is_none());
    assert_eq!(
        should_moderate_characters("Привет", 5),
        some("6 characters")
    );

    // Emoji scalar value (1 char, 4 bytes)
    assert!(should_moderate_characters("🦀", 1).is_none());
    assert_eq!(should_moderate_characters("🦀🦀", 1), some("2 characters"));

    // Whitespace and newlines count toward max_characters
    assert!(should_moderate_characters("a\n b", 4).is_none());
    assert_eq!(should_moderate_characters("a\n b", 3), some("4 characters"));
}

#[test]
fn test_leading_and_trailing_space_floods_count() {
    // Dot at start, 500 spaces, NO dot at end
    let spaces_trailing = format!(".{}", " ".repeat(500));
    assert_eq!(
        should_moderate_characters(&spaces_trailing, 100),
        some("501 characters")
    );

    // 500 spaces, dot at end
    let spaces_leading = format!("{}.", " ".repeat(500));
    assert_eq!(
        should_moderate_characters(&spaces_leading, 100),
        some("501 characters")
    );
}

#[test]
fn test_zero_maximum_never_matches() {
    // Validation rejects 0 on save; a stored 0 must not match every message.
    let large_message = "word ".repeat(100) + "\n".repeat(50).as_str() + &"a".repeat(500);
    assert!(should_moderate_characters(&large_message, 0).is_none());
}

#[test]
fn test_rejects_a_zero_maximum() {
    // 0 used to mean "this check is off" inside the old combined condition;
    // on its own it would store a rule that can never match.
    let err = ExceedsMaxCharacters { max_characters: 0 }
        .normalize_and_validate()
        .expect_err("a zero maximum should have been rejected")
        .to_string();
    assert!(err.contains("'Message Exceeds Max Characters' needs a maximum of at least 1"));
}
