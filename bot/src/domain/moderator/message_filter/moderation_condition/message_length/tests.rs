use super::{should_moderate_characters, should_moderate_lines, should_moderate_words};

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
fn test_max_lines_without_chars_per_line() {
    // Exact boundaries around limit = 3
    assert!(should_moderate_lines("line 1\nline 2", 3, 0).is_none());
    assert!(should_moderate_lines("line 1\nline 2\nline 3", 3, 0).is_none());
    assert_eq!(
        should_moderate_lines("line 1\nline 2\nline 3\nline 4", 3, 0),
        some("4 lines")
    );

    // CRLF (\r\n) linebreaks
    assert!(should_moderate_lines("line 1\r\nline 2\r\nline 3", 3, 0).is_none());
    assert_eq!(
        should_moderate_lines("line 1\r\nline 2\r\nline 3\r\nline 4", 3, 0),
        some("4 lines")
    );

    // Consecutive empty lines count as lines
    assert!(should_moderate_lines("a\n\nb", 3, 0).is_none()); // 3 lines
    assert_eq!(should_moderate_lines("a\n\n\nb", 3, 0), some("4 lines"));

    // Trailing newline adds a line
    assert!(should_moderate_lines("a\nb\n", 3, 0).is_none()); // 3 lines
    assert_eq!(should_moderate_lines("a\nb\nc\n", 3, 0), some("4 lines"));

    // Leading newline adds a line
    assert!(should_moderate_lines("\na\nb", 3, 0).is_none()); // 3 lines
    assert_eq!(should_moderate_lines("\n\na\nb", 3, 0), some("4 lines"));

    // Alternative Unicode line breaks (bare CR, VT, FF, NEL, LS, PS)
    assert!(should_moderate_lines("a\rb\rc", 3, 0).is_none());
    assert_eq!(should_moderate_lines("a\rb\rc\rd", 3, 0), some("4 lines"));
    assert_eq!(
        should_moderate_lines("a\x0Bb\x0Cc\u{0085}d", 3, 0),
        some("4 lines")
    );
    assert_eq!(
        should_moderate_lines("a\u{2028}b\u{2029}c\u{2028}d", 3, 0),
        some("4 lines")
    );

    // A blank message made of line breaks still has lines
    assert_eq!(
        should_moderate_lines(&"\n".repeat(10_000), 5, 0),
        some("10001 lines")
    );
}

#[test]
fn test_max_lines_with_chars_per_line() {
    // 10 chars -> ceil(10/10) = 1 line
    assert!(should_moderate_lines(&"a".repeat(10), 1, 10).is_none());
    // 11 chars -> ceil(11/10) = 2 lines
    assert!(should_moderate_lines(&"a".repeat(11), 2, 10).is_none());
    assert_eq!(
        should_moderate_lines(&"a".repeat(11), 1, 10),
        some("2 lines")
    );

    // 20 chars -> ceil(20/10) = 2 lines
    assert!(should_moderate_lines(&"a".repeat(20), 2, 10).is_none());
    // 21 chars -> ceil(21/10) = 3 lines
    assert!(should_moderate_lines(&"a".repeat(21), 3, 10).is_none());
    assert_eq!(
        should_moderate_lines(&"a".repeat(21), 2, 10),
        some("3 lines")
    );

    // 30 chars in 1 line -> ceil(30 / 10) = 3 lines <= 3 -> no match
    assert!(should_moderate_lines(&"a".repeat(30), 3, 10).is_none());
    // 31 chars in 1 line -> ceil(31 / 10) = 4 lines > 3 -> match
    assert_eq!(
        should_moderate_lines(&"a".repeat(31), 3, 10),
        some("4 lines")
    );

    // Multibyte Unicode wrap: 10 Cyrillic characters count as 10 characters, not 20 bytes
    assert!(should_moderate_lines("абвгдеёжзи", 1, 10).is_none());
    assert_eq!(should_moderate_lines("абвгдеёжзий", 1, 10), some("2 lines"));

    // Empty line with soft wrap counts as 1 line
    let with_empty_line = format!("{}\n\n{}", "a".repeat(10), "b".repeat(10)); // 1 + 1 + 1 = 3 lines
    assert!(should_moderate_lines(&with_empty_line, 3, 10).is_none());
    assert_eq!(
        should_moderate_lines(&with_empty_line, 2, 10),
        some("3 lines")
    );

    // Multi-line wrap calculation:
    // line 1: 15 chars (ceil(15/10) = 2 lines)
    // line 2: 12 chars (ceil(12/10) = 2 lines)
    // total = 4 lines > 3 -> match
    let multi = format!("{}\n{}", "a".repeat(15), "b".repeat(12));
    assert_eq!(should_moderate_lines(&multi, 3, 10), some("4 lines"));

    // chars_per_line = 1 (every character wraps)
    assert!(should_moderate_lines("abc", 3, 1).is_none());
    assert_eq!(should_moderate_lines("abcd", 3, 1), some("4 lines"));

    // chars_per_line = 0 disables soft wrapping
    let line_50 = "a".repeat(50);
    assert!(should_moderate_lines(&line_50, 3, 10).is_some());
    assert!(should_moderate_lines(&line_50, 3, 0).is_none());
}

#[test]
fn test_leading_and_trailing_flood_edge_cases() {
    // Dot at start, 500 newlines, dot at end
    let flood_both = format!(".{}.", "\n".repeat(500));
    assert_eq!(should_moderate_lines(&flood_both, 5, 0), some("501 lines"));

    // Dot at start, 500 newlines, NO dot at end (trailing flood)
    let flood_trailing = format!(".{}", "\n".repeat(500));
    assert_eq!(
        should_moderate_lines(&flood_trailing, 5, 0),
        some("501 lines")
    );

    // 500 newlines, dot at end (leading flood)
    let flood_leading = format!("{}.", "\n".repeat(500));
    assert_eq!(
        should_moderate_lines(&flood_leading, 5, 0),
        some("501 lines")
    );

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
    assert!(should_moderate_words(&large_message, 0).is_none());
    assert!(should_moderate_lines(&large_message, 0, 0).is_none());
    assert!(should_moderate_lines(&large_message, 0, 10).is_none());
}

#[tokio::test]
async fn test_integration_with_message_filter_rules() {
    use crate::domain::moderator::message_filter::{
        ModerationAction, ModerationCondition, ModerationRule,
        should_moderate as top_level_moderate,
    };
    use crate::domain::moderator::ports::GroupMessage;
    use crate::infrastructure::adapters::user_activity_repo_in_memory::InMemoryUserActivityRepository;
    use crate::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;

    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let msg = |text: &str| GroupMessage {
        text: text.to_string(),
        ..Default::default()
    };

    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::IsBlank,
                ModerationCondition::ExceedsMaxCharacters {
                    max_characters: 100,
                },
                ModerationCondition::ExceedsMaxWords { max_words: 10 },
                ModerationCondition::ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 40,
                },
            ],
        },
    }];

    let matches = async |text: &str, rules: &[ModerationRule]| {
        top_level_moderate(&msg(text), rules, &repo, &mod_repo)
            .await
            .unwrap()
            .is_some()
    };

    // Normal message passes
    assert!(!matches("Hello world", &rules).await);

    // Leading and trailing newlines are NOT stripped by a top-level trim
    assert!(matches(&format!(".{}", "\n".repeat(500)), &rules).await);
    assert!(matches(&format!("{}.", "\n".repeat(500)), &rules).await);

    // 500 trailing spaces exceed max_characters
    assert!(matches(&format!(".{}", " ".repeat(500)), &rules).await);

    // A blank message matches through IsBlank
    assert!(matches("   ", &rules).await);

    // Without IsBlank, a short blank message matches nothing
    let length_only = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::ExceedsMaxCharacters {
            max_characters: 100,
        },
    }];
    assert!(!matches("   ", &length_only).await);
}
