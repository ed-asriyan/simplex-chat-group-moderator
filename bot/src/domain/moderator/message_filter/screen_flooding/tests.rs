use super::should_moderate;

#[test]
fn test_empty_string() {
    // With disallow_empty_messages = true (default)
    assert_eq!(
        should_moderate("", None, None, None, None, false, true),
        Some(String::new())
    );
    // With disallow_empty_messages = false
    assert!(should_moderate("", None, None, None, None, false, false).is_none());
}

#[test]
fn test_whitespace_only() {
    assert_eq!(
        should_moderate("   ", None, None, None, None, false, true),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("\n\n\t \r\n", None, None, None, None, false, true),
        Some(String::new())
    );

    // If disallow_empty_messages = false, whitespace-only messages are allowed unless another rule catches them
    assert!(should_moderate("   ", None, None, None, None, false, false).is_none());
    assert!(should_moderate("\n\n\t \r\n", None, None, None, None, false, false).is_none());
}

#[test]
fn test_many_newlines_only() {
    let message = "\n".repeat(10_000);
    assert_eq!(
        should_moderate(&message, None, None, None, None, false, true),
        Some(String::new())
    );
    // Even if disallow_empty_messages = false, max_lines will still catch it
    assert_eq!(
        should_moderate(&message, None, None, Some(5), None, false, false),
        Some(String::new())
    );
    // If both disallow_empty_messages = false and no other limits, it is allowed
    assert!(should_moderate(&message, None, None, None, None, false, false).is_none());
}

#[test]
fn test_invisible_characters_only() {
    assert_eq!(
        should_moderate(
            "\u{200B}\u{200B}\u{200B}",
            None,
            None,
            None,
            None,
            false,
            true
        ),
        Some(String::new())
    );
    assert_eq!(
        should_moderate(
            "  \u{FEFF}\u{200C}\n\u{200D}\u{2060} \u{00AD} ",
            None,
            None,
            None,
            None,
            false,
            true,
        ),
        Some(String::new())
    );

    // If disallow_empty_messages = false, but disallow_invisible_chars = true, still caught!
    assert_eq!(
        should_moderate(
            "\u{200B}\u{200B}\u{200B}",
            None,
            None,
            None,
            None,
            true,
            false
        ),
        Some(String::new())
    );
}

#[test]
fn test_braille_blank_pattern() {
    assert_eq!(
        should_moderate("⠀", None, None, None, None, false, true),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("⠀\n⠀\n\n⠀", None, None, None, None, false, true),
        Some(String::new())
    );

    // If disallow_empty_messages = false and disallow_invisible_chars = false
    assert!(should_moderate("⠀", None, None, None, None, false, false).is_none());
}

#[test]
fn test_bidi_and_variation_selectors_only() {
    assert_eq!(
        should_moderate(
            "\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}",
            None,
            None,
            None,
            None,
            false,
            true,
        ),
        Some(String::new())
    );
    assert_eq!(
        should_moderate(
            "\u{FE00}\u{FE0F}\u{E0100}",
            None,
            None,
            None,
            None,
            false,
            true
        ),
        Some(String::new())
    );
}

#[test]
fn test_control_characters_only() {
    assert_eq!(
        should_moderate(
            "\u{0000}\u{0007}\u{001B}\u{007F}\u{0080}\u{009F}",
            None,
            None,
            None,
            None,
            false,
            true,
        ),
        Some(String::new())
    );
}

#[test]
fn test_non_empty_message_allowed_without_limits() {
    assert!(should_moderate("hello", None, None, None, None, false, true).is_none());
    assert!(should_moderate("  hello  ", None, None, None, None, false, true).is_none());
    // Invisible characters are allowed if disallow_invisible_chars = false
    assert!(
        should_moderate("\u{200B}hello\u{200B}", None, None, None, None, false, true).is_none()
    );
}

#[test]
fn test_max_characters() {
    // Exact boundaries around limit = 10
    assert!(should_moderate("012345678", Some(10), None, None, None, false, true).is_none());
    assert!(should_moderate("0123456789", Some(10), None, None, None, false, true).is_none());
    assert_eq!(
        should_moderate("0123456789a", Some(10), None, None, None, false, true),
        Some(String::new())
    );

    // Multibyte Unicode characters (e.g. Cyrillic: 6 characters = 12 UTF-8 bytes)
    assert!(should_moderate("Привет", Some(6), None, None, None, false, true).is_none());
    assert_eq!(
        should_moderate("Привет", Some(5), None, None, None, false, true),
        Some(String::new())
    );

    // Emoji scalar value (1 char, 4 bytes)
    assert!(should_moderate("🦀", Some(1), None, None, None, false, true).is_none());
    assert_eq!(
        should_moderate("🦀🦀", Some(1), None, None, None, false, true),
        Some(String::new())
    );

    // Whitespace and newlines count toward max_characters
    assert!(should_moderate("a\n b", Some(4), None, None, None, false, true).is_none());
    assert_eq!(
        should_moderate("a\n b", Some(3), None, None, None, false, true),
        Some(String::new())
    );
}

#[test]
fn test_max_words() {
    // Exact boundaries around limit = 3
    assert!(should_moderate("one two", None, Some(3), None, None, false, true).is_none());
    assert!(should_moderate("one two three", None, Some(3), None, None, false, true).is_none());
    assert_eq!(
        should_moderate("one two three four", None, Some(3), None, None, false, true),
        Some(String::new())
    );

    // Multiple irregular whitespace characters (spaces, tabs, newlines)
    assert!(
        should_moderate(
            "  one \t\t\n  two \n\n  three  ",
            None,
            Some(3),
            None,
            None,
            false,
            true
        )
        .is_none()
    );
    assert_eq!(
        should_moderate(
            "  one \t\t\n  two \n\n  three \n four ",
            None,
            Some(3),
            None,
            None,
            false,
            true
        ),
        Some(String::new())
    );

    // Words with punctuation
    assert!(
        should_moderate(
            "Hello, world! How are you?",
            None,
            Some(5),
            None,
            None,
            false,
            true
        )
        .is_none()
    );
    assert_eq!(
        should_moderate(
            "Hello, world! How are you?",
            None,
            Some(4),
            None,
            None,
            false,
            true
        ),
        Some(String::new())
    );

    // Single very long word counts as 1 word
    let long_word = "a".repeat(1000);
    assert!(should_moderate(&long_word, None, Some(1), None, None, false, true).is_none());
}

#[test]
fn test_max_lines_without_chars_per_line() {
    // Exact boundaries around limit = 3
    assert!(should_moderate("line 1\nline 2", None, None, Some(3), None, false, true).is_none());
    assert!(
        should_moderate(
            "line 1\nline 2\nline 3",
            None,
            None,
            Some(3),
            None,
            false,
            true
        )
        .is_none()
    );
    assert_eq!(
        should_moderate(
            "line 1\nline 2\nline 3\nline 4",
            None,
            None,
            Some(3),
            None,
            false,
            true
        ),
        Some(String::new())
    );

    // CRLF (\r\n) linebreaks
    assert!(
        should_moderate(
            "line 1\r\nline 2\r\nline 3",
            None,
            None,
            Some(3),
            None,
            false,
            true
        )
        .is_none()
    );
    assert_eq!(
        should_moderate(
            "line 1\r\nline 2\r\nline 3\r\nline 4",
            None,
            None,
            Some(3),
            None,
            false,
            true
        ),
        Some(String::new())
    );

    // Consecutive empty lines count as lines
    assert!(should_moderate("a\n\nb", None, None, Some(3), None, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("a\n\n\nb", None, None, Some(3), None, false, true), // 4 lines
        Some(String::new())
    );

    // Trailing newline adds a line
    assert!(should_moderate("a\nb\n", None, None, Some(3), None, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("a\nb\nc\n", None, None, Some(3), None, false, true), // 4 lines
        Some(String::new())
    );

    // Leading newline adds a line
    assert!(should_moderate("\na\nb", None, None, Some(3), None, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("\n\na\nb", None, None, Some(3), None, false, true), // 4 lines
        Some(String::new())
    );
}

#[test]
fn test_max_lines_with_chars_per_line() {
    // chars_per_line = 10, max_lines = 3.
    // Exact multiples of wrap width:
    // 10 chars -> ceil(10/10) = 1 line
    assert!(should_moderate(&"a".repeat(10), None, None, Some(1), Some(10), false, true).is_none());
    // 11 chars -> ceil(11/10) = 2 lines
    assert!(should_moderate(&"a".repeat(11), None, None, Some(2), Some(10), false, true).is_none());
    assert_eq!(
        should_moderate(&"a".repeat(11), None, None, Some(1), Some(10), false, true),
        Some(String::new())
    );

    // 20 chars -> ceil(20/10) = 2 lines
    assert!(should_moderate(&"a".repeat(20), None, None, Some(2), Some(10), false, true).is_none());
    // 21 chars -> ceil(21/10) = 3 lines
    assert!(should_moderate(&"a".repeat(21), None, None, Some(3), Some(10), false, true).is_none());
    assert_eq!(
        should_moderate(&"a".repeat(21), None, None, Some(2), Some(10), false, true),
        Some(String::new())
    );

    // 30 chars in 1 line -> ceil(30 / 10) = 3 lines <= 3 -> allowed
    let text_30 = "a".repeat(30);
    assert!(should_moderate(&text_30, None, None, Some(3), Some(10), false, true).is_none());

    // 31 chars in 1 line -> ceil(31 / 10) = 4 lines > 3 -> moderated
    let text_31 = "a".repeat(31);
    assert_eq!(
        should_moderate(&text_31, None, None, Some(3), Some(10), false, true),
        Some(String::new())
    );

    // Multibyte Unicode wrap: 10 Cyrillic characters count as 10 characters, not 20 bytes
    let cyrillic_10 = "абвгдеёжзи";
    assert!(should_moderate(cyrillic_10, None, None, Some(1), Some(10), false, true).is_none());
    let cyrillic_11 = "абвгдеёжзий";
    assert_eq!(
        should_moderate(cyrillic_11, None, None, Some(1), Some(10), false, true),
        Some(String::new())
    );

    // Empty line with soft wrap counts as 1 line
    let with_empty_line = format!("{}\n\n{}", "a".repeat(10), "b".repeat(10)); // 1 + 1 + 1 = 3 lines
    assert!(
        should_moderate(&with_empty_line, None, None, Some(3), Some(10), false, true).is_none()
    );
    assert_eq!(
        should_moderate(&with_empty_line, None, None, Some(2), Some(10), false, true),
        Some(String::new())
    );

    // Multi-line wrap calculation:
    // line 1: 15 chars (ceil(15/10) = 2 lines)
    // line 2: 12 chars (ceil(12/10) = 2 lines)
    // total = 4 lines > 3 -> moderated
    let multi = format!("{}\n{}", "a".repeat(15), "b".repeat(12));
    assert_eq!(
        should_moderate(&multi, None, None, Some(3), Some(10), false, true),
        Some(String::new())
    );

    // chars_per_line = 1 (every character wraps)
    assert!(should_moderate("abc", None, None, Some(3), Some(1), false, true).is_none());
    assert_eq!(
        should_moderate("abcd", None, None, Some(3), Some(1), false, true),
        Some(String::new())
    );
}

#[test]
fn test_disallow_invisible_chars() {
    // Normal text with regular whitespace (spaces, tabs, newlines, carriage returns) is allowed
    assert!(
        should_moderate(
            "hello world\nline 2\tindent\r\n",
            None,
            None,
            None,
            None,
            true,
            true
        )
        .is_none()
    );

    // Even a single zero-width space anywhere in otherwise normal text is banned
    assert_eq!(
        should_moderate("\u{200B}hello world", None, None, None, None, true, true),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{200B}world", None, None, None, None, true, true),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello world\u{200B}", None, None, None, None, true, true),
        Some(String::new())
    );

    // Other specific invisible characters
    assert_eq!(
        should_moderate("hello⠀world", None, None, None, None, true, true), // Braille blank U+2800
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{00AD}world", None, None, None, None, true, true), // soft hyphen
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{180E}world", None, None, None, None, true, true), // Mongolian vowel separator
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{200C}world", None, None, None, None, true, true), // zero width non-joiner
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{200D}world", None, None, None, None, true, true), // zero width joiner
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{202E}world", None, None, None, None, true, true), // Bidi override
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{2060}world", None, None, None, None, true, true), // word joiner
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{2066}world", None, None, None, None, true, true), // Bidi isolate
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{FE00}world", None, None, None, None, true, true), // Variation Selector
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{E0100}world", None, None, None, None, true, true), // Variation Selector supplement
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{E0020}world", None, None, None, None, true, true), // Unicode Tag char
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{FFF9}world", None, None, None, None, true, true), // interlinear annotation
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{0000}world", None, None, None, None, true, true), // NUL control char
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{0007}world", None, None, None, None, true, true), // BEL control char
        Some(String::new())
    );
    assert_eq!(
        should_moderate("hello\u{001B}world", None, None, None, None, true, true), // ESC control char
        Some(String::new())
    );

    // When disallow_invisible_chars = false, normal text with invisible characters is allowed
    assert!(should_moderate("hello\u{200B}world", None, None, None, None, false, true).is_none());
    assert!(should_moderate("hello⠀world", None, None, None, None, false, true).is_none());
}

#[test]
fn test_leading_and_trailing_flood_edge_cases() {
    // Dot at start, 500 newlines, dot at end
    let flood_both = format!(".{}.", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_both, None, None, Some(5), None, false, true),
        Some(String::new())
    );

    // Dot at start, 500 newlines, NO dot at end (trailing flood)
    let flood_trailing = format!(".{}", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_trailing, None, None, Some(5), None, false, true),
        Some(String::new())
    );

    // 500 newlines, dot at end (leading flood)
    let flood_leading = format!("{}.", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_leading, None, None, Some(5), None, false, true),
        Some(String::new())
    );

    // Dot at start, 500 spaces, NO dot at end
    let spaces_trailing = format!(".{}", " ".repeat(500));
    assert_eq!(
        should_moderate(&spaces_trailing, Some(100), None, None, None, false, true),
        Some(String::new())
    );

    // 500 spaces, dot at end
    let spaces_leading = format!("{}.", " ".repeat(500));
    assert_eq!(
        should_moderate(&spaces_leading, Some(100), None, None, None, false, true),
        Some(String::new())
    );
}

#[test]
fn test_integration_with_message_filter_rules() {
    use crate::domain::moderator::message_filter::{
        ModerationRule, should_moderate as top_level_moderate,
    };

    let rules = vec![ModerationRule::ScreenFlooding {
        max_characters: Some(100),
        max_words: Some(10),
        max_lines: Some(5),
        chars_per_line: Some(40),
        disallow_invisible_chars: false,
        disallow_empty_messages: true,
    }];

    // Normal message passes
    assert!(top_level_moderate("Hello world", &rules).is_none());

    // Message with trailing 500 newlines is NOT stripped by top-level trim and gets moderated!
    let trailing_newlines = format!(".{}", "\n".repeat(500));
    assert!(top_level_moderate(&trailing_newlines, &rules).is_some());

    // Message with leading 500 newlines is NOT stripped by top-level trim and gets moderated!
    let leading_newlines = format!("{}.", "\n".repeat(500));
    assert!(top_level_moderate(&leading_newlines, &rules).is_some());

    // Message with 500 trailing spaces exceeds max_characters
    let trailing_spaces = format!(".{}", " ".repeat(500));
    assert!(top_level_moderate(&trailing_spaces, &rules).is_some());

    // Empty message gets moderated because disallow_empty_messages = true
    assert!(top_level_moderate("   ", &rules).is_some());

    // With disallow_empty_messages = false and no limits exceeded
    let rules_no_empty_ban = vec![ModerationRule::ScreenFlooding {
        max_characters: None,
        max_words: None,
        max_lines: None,
        chars_per_line: None,
        disallow_invisible_chars: false,
        disallow_empty_messages: false,
    }];
    assert!(top_level_moderate("   ", &rules_no_empty_ban).is_none());
}

#[test]
fn test_zero_limits_treated_as_unlimited() {
    // A message with lots of characters, words, lines and soft-wrapping:
    let large_message = "word ".repeat(100) + "\n".repeat(50).as_str() + &"a".repeat(500);

    // 1. Check with explicit positive limits to verify it indeed gets moderated:
    assert!(should_moderate(&large_message, Some(10), None, None, None, false, true).is_some());
    assert!(should_moderate(&large_message, None, Some(10), None, None, false, true).is_some());
    assert!(should_moderate(&large_message, None, None, Some(10), None, false, true).is_some());

    // 2. Setting each limit individually to 0 disables it:
    assert!(should_moderate(&large_message, Some(0), None, None, None, false, true).is_none());
    assert!(should_moderate(&large_message, None, Some(0), None, None, false, true).is_none());
    assert!(should_moderate(&large_message, None, None, Some(0), None, false, true).is_none());

    // 3. Setting all limits to 0 simultaneously:
    assert!(
        should_moderate(
            &large_message,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            false,
            true
        )
        .is_none()
    );

    // 4. chars_per_line = 0 disables soft wrapping:
    let line_50 = "a".repeat(50);
    assert!(should_moderate(&line_50, None, None, Some(3), Some(10), false, true).is_some());
    assert!(should_moderate(&line_50, None, None, Some(3), Some(0), false, true).is_none());

    // 5. Completely empty/invisible message is still moderated when disallow_empty_messages = true even if limits are 0:
    assert_eq!(
        should_moderate("   ", Some(0), Some(0), Some(0), Some(0), false, true),
        Some(String::new())
    );

    // 6. When disallow_empty_messages = false, empty message is NOT moderated:
    assert!(should_moderate("   ", Some(0), Some(0), Some(0), Some(0), false, false).is_none());
}
