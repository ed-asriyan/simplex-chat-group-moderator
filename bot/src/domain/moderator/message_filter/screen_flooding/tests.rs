use super::should_moderate;

#[test]
fn test_empty_string() {
    // With disallow_empty_messages = true (default)
    assert_eq!(
        should_moderate("", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
    // With disallow_empty_messages = false
    assert!(should_moderate("", 0, 0, 0, 0, false, false).is_none());
}

#[test]
fn test_whitespace_only() {
    assert_eq!(
        should_moderate("   ", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
    assert_eq!(
        should_moderate("\n\n\t \r\n", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );

    // If disallow_empty_messages = false, whitespace-only messages are allowed unless another rule catches them
    assert!(should_moderate("   ", 0, 0, 0, 0, false, false).is_none());
    assert!(should_moderate("\n\n\t \r\n", 0, 0, 0, 0, false, false).is_none());
}

#[test]
fn test_many_newlines_only() {
    let message = "\n".repeat(10_000);
    assert_eq!(
        should_moderate(&message, 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
    // Even if disallow_empty_messages = false, max_lines will still catch it
    assert_eq!(
        should_moderate(&message, 0, 0, 5, 0, false, false),
        Some("10001 lines".to_string())
    );
    // If both disallow_empty_messages = false and no other limits, it is allowed
    assert!(should_moderate(&message, 0, 0, 0, 0, false, false).is_none());
}

#[test]
fn test_invisible_characters_only() {
    assert_eq!(
        should_moderate("\u{200B}\u{200B}\u{200B}", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
    assert_eq!(
        should_moderate(
            "  \u{FEFF}\u{200C}\n\u{200D}\u{2060} \u{00AD} ",
            0,
            0,
            0,
            0,
            false,
            true,
        ),
        Some("empty message".to_string())
    );

    // If disallow_empty_messages = false, but disallow_invisible_chars = true, still caught!
    assert_eq!(
        should_moderate("\u{200B}\u{200B}\u{200B}", 0, 0, 0, 0, true, false),
        Some("invisible characters".to_string())
    );
}

#[test]
fn test_braille_blank_pattern() {
    assert_eq!(
        should_moderate("⠀", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
    assert_eq!(
        should_moderate("⠀\n⠀\n\n⠀", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );

    // If disallow_empty_messages = false and disallow_invisible_chars = false
    assert!(should_moderate("⠀", 0, 0, 0, 0, false, false).is_none());
}

#[test]
fn test_bidi_and_variation_selectors_only() {
    assert_eq!(
        should_moderate(
            "\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}",
            0,
            0,
            0,
            0,
            false,
            true,
        ),
        Some("empty message".to_string())
    );
    assert_eq!(
        should_moderate("\u{FE00}\u{FE0F}\u{E0100}", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );
}

#[test]
fn test_control_characters_only() {
    assert_eq!(
        should_moderate(
            "\u{0000}\u{0007}\u{001B}\u{007F}\u{0080}\u{009F}",
            0,
            0,
            0,
            0,
            false,
            true,
        ),
        Some("empty message".to_string())
    );
}

#[test]
fn test_non_empty_message_allowed_without_limits() {
    assert!(should_moderate("hello", 0, 0, 0, 0, false, true).is_none());
    assert!(should_moderate("  hello  ", 0, 0, 0, 0, false, true).is_none());
    // Invisible characters are allowed if disallow_invisible_chars = false
    assert!(should_moderate("\u{200B}hello\u{200B}", 0, 0, 0, 0, false, true).is_none());
}

#[test]
fn test_max_characters() {
    // Exact boundaries around limit = 10
    assert!(should_moderate("012345678", 10, 0, 0, 0, false, true).is_none());
    assert!(should_moderate("0123456789", 10, 0, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("0123456789a", 10, 0, 0, 0, false, true),
        Some("11 characters".to_string())
    );

    // Multibyte Unicode characters (e.g. Cyrillic: 6 characters = 12 UTF-8 bytes)
    assert!(should_moderate("Привет", 6, 0, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("Привет", 5, 0, 0, 0, false, true),
        Some("6 characters".to_string())
    );

    // Emoji scalar value (1 char, 4 bytes)
    assert!(should_moderate("🦀", 1, 0, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("🦀🦀", 1, 0, 0, 0, false, true),
        Some("2 characters".to_string())
    );

    // Whitespace and newlines count toward max_characters
    assert!(should_moderate("a\n b", 4, 0, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("a\n b", 3, 0, 0, 0, false, true),
        Some("4 characters".to_string())
    );
}

#[test]
fn test_max_words() {
    // Exact boundaries around limit = 3
    assert!(should_moderate("one two", 0, 3, 0, 0, false, true).is_none());
    assert!(should_moderate("one two three", 0, 3, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("one two three four", 0, 3, 0, 0, false, true),
        Some("4 words".to_string())
    );

    // Multiple irregular whitespace characters (spaces, tabs, newlines)
    assert!(should_moderate("  one \t\t\n  two \n\n  three  ", 0, 3, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate(
            "  one \t\t\n  two \n\n  three \n four ",
            0,
            3,
            0,
            0,
            false,
            true
        ),
        Some("4 words".to_string())
    );

    // Words with punctuation
    assert!(should_moderate("Hello, world! How are you?", 0, 5, 0, 0, false, true).is_none());
    assert_eq!(
        should_moderate("Hello, world! How are you?", 0, 4, 0, 0, false, true),
        Some("5 words".to_string())
    );

    // Single very long word counts as 1 word
    let long_word = "a".repeat(1000);
    assert!(should_moderate(&long_word, 0, 1, 0, 0, false, true).is_none());
}

#[test]
fn test_max_lines_without_chars_per_line() {
    // Exact boundaries around limit = 3
    assert!(should_moderate("line 1\nline 2", 0, 0, 3, 0, false, true).is_none());
    assert!(should_moderate("line 1\nline 2\nline 3", 0, 0, 3, 0, false, true).is_none());
    assert_eq!(
        should_moderate("line 1\nline 2\nline 3\nline 4", 0, 0, 3, 0, false, true),
        Some("4 lines".to_string())
    );

    // CRLF (\r\n) linebreaks
    assert!(should_moderate("line 1\r\nline 2\r\nline 3", 0, 0, 3, 0, false, true).is_none());
    assert_eq!(
        should_moderate(
            "line 1\r\nline 2\r\nline 3\r\nline 4",
            0,
            0,
            3,
            0,
            false,
            true
        ),
        Some("4 lines".to_string())
    );

    // Consecutive empty lines count as lines
    assert!(should_moderate("a\n\nb", 0, 0, 3, 0, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("a\n\n\nb", 0, 0, 3, 0, false, true), // 4 lines
        Some("4 lines".to_string())
    );

    // Trailing newline adds a line
    assert!(should_moderate("a\nb\n", 0, 0, 3, 0, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("a\nb\nc\n", 0, 0, 3, 0, false, true), // 4 lines
        Some("4 lines".to_string())
    );

    // Leading newline adds a line
    assert!(should_moderate("\na\nb", 0, 0, 3, 0, false, true).is_none()); // 3 lines
    assert_eq!(
        should_moderate("\n\na\nb", 0, 0, 3, 0, false, true), // 4 lines
        Some("4 lines".to_string())
    );

    // Alternative Unicode line breaks (bare CR, VT, FF, NEL, LS, PS)
    assert!(should_moderate("a\rb\rc", 0, 0, 3, 0, false, true).is_none());
    assert_eq!(
        should_moderate("a\rb\rc\rd", 0, 0, 3, 0, false, true),
        Some("4 lines".to_string())
    );
    assert_eq!(
        should_moderate("a\x0Bb\x0Cc\u{0085}d", 0, 0, 3, 0, false, true),
        Some("4 lines".to_string())
    );
    assert_eq!(
        should_moderate("a\u{2028}b\u{2029}c\u{2028}d", 0, 0, 3, 0, false, true),
        Some("4 lines".to_string())
    );
}

#[test]
fn test_max_lines_with_chars_per_line() {
    // chars_per_line = 10, max_lines = 3.
    // Exact multiples of wrap width:
    // 10 chars -> ceil(10/10) = 1 line
    assert!(should_moderate(&"a".repeat(10), 0, 0, 1, 10, false, true).is_none());
    // 11 chars -> ceil(11/10) = 2 lines
    assert!(should_moderate(&"a".repeat(11), 0, 0, 2, 10, false, true).is_none());
    assert_eq!(
        should_moderate(&"a".repeat(11), 0, 0, 1, 10, false, true),
        Some("2 lines".to_string())
    );

    // 20 chars -> ceil(20/10) = 2 lines
    assert!(should_moderate(&"a".repeat(20), 0, 0, 2, 10, false, true).is_none());
    // 21 chars -> ceil(21/10) = 3 lines
    assert!(should_moderate(&"a".repeat(21), 0, 0, 3, 10, false, true).is_none());
    assert_eq!(
        should_moderate(&"a".repeat(21), 0, 0, 2, 10, false, true),
        Some("3 lines".to_string())
    );

    // 30 chars in 1 line -> ceil(30 / 10) = 3 lines <= 3 -> allowed
    let text_30 = "a".repeat(30);
    assert!(should_moderate(&text_30, 0, 0, 3, 10, false, true).is_none());

    // 31 chars in 1 line -> ceil(31 / 10) = 4 lines > 3 -> moderated
    let text_31 = "a".repeat(31);
    assert_eq!(
        should_moderate(&text_31, 0, 0, 3, 10, false, true),
        Some("4 lines".to_string())
    );

    // Multibyte Unicode wrap: 10 Cyrillic characters count as 10 characters, not 20 bytes
    let cyrillic_10 = "абвгдеёжзи";
    assert!(should_moderate(cyrillic_10, 0, 0, 1, 10, false, true).is_none());
    let cyrillic_11 = "абвгдеёжзий";
    assert_eq!(
        should_moderate(cyrillic_11, 0, 0, 1, 10, false, true),
        Some("2 lines".to_string())
    );

    // Empty line with soft wrap counts as 1 line
    let with_empty_line = format!("{}\n\n{}", "a".repeat(10), "b".repeat(10)); // 1 + 1 + 1 = 3 lines
    assert!(should_moderate(&with_empty_line, 0, 0, 3, 10, false, true).is_none());
    assert_eq!(
        should_moderate(&with_empty_line, 0, 0, 2, 10, false, true),
        Some("3 lines".to_string())
    );

    // Multi-line wrap calculation:
    // line 1: 15 chars (ceil(15/10) = 2 lines)
    // line 2: 12 chars (ceil(12/10) = 2 lines)
    // total = 4 lines > 3 -> moderated
    let multi = format!("{}\n{}", "a".repeat(15), "b".repeat(12));
    assert_eq!(
        should_moderate(&multi, 0, 0, 3, 10, false, true),
        Some("4 lines".to_string())
    );

    // chars_per_line = 1 (every character wraps)
    assert!(should_moderate("abc", 0, 0, 3, 1, false, true).is_none());
    assert_eq!(
        should_moderate("abcd", 0, 0, 3, 1, false, true),
        Some("4 lines".to_string())
    );
}

#[test]
fn test_disallow_invisible_chars() {
    // Normal text with regular whitespace (spaces, tabs, newlines, carriage returns) is allowed
    assert!(should_moderate("hello world\nline 2\tindent\r\n", 0, 0, 0, 0, true, true).is_none());

    // Even a single zero-width space anywhere in otherwise normal text is banned
    assert_eq!(
        should_moderate("\u{200B}hello world", 0, 0, 0, 0, true, true),
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{200B}world", 0, 0, 0, 0, true, true),
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello world\u{200B}", 0, 0, 0, 0, true, true),
        Some("invisible characters".to_string())
    );

    // Other specific invisible characters
    assert_eq!(
        should_moderate("hello⠀world", 0, 0, 0, 0, true, true), // Braille blank U+2800
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{00AD}world", 0, 0, 0, 0, true, true), // soft hyphen
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{180E}world", 0, 0, 0, 0, true, true), // Mongolian vowel separator
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{200C}world", 0, 0, 0, 0, true, true), // zero width non-joiner
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{200D}world", 0, 0, 0, 0, true, true), // zero width joiner
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{202E}world", 0, 0, 0, 0, true, true), // Bidi override
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{2060}world", 0, 0, 0, 0, true, true), // word joiner
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{2066}world", 0, 0, 0, 0, true, true), // Bidi isolate
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{FE00}world", 0, 0, 0, 0, true, true), // Variation Selector
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{E0100}world", 0, 0, 0, 0, true, true), // Variation Selector supplement
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{E0020}world", 0, 0, 0, 0, true, true), // Unicode Tag char
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{FFF9}world", 0, 0, 0, 0, true, true), // interlinear annotation
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{0000}world", 0, 0, 0, 0, true, true), // NUL control char
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{0007}world", 0, 0, 0, 0, true, true), // BEL control char
        Some("invisible characters".to_string())
    );
    assert_eq!(
        should_moderate("hello\u{001B}world", 0, 0, 0, 0, true, true), // ESC control char
        Some("invisible characters".to_string())
    );

    // When disallow_invisible_chars = false, normal text with invisible characters is allowed
    assert!(should_moderate("hello\u{200B}world", 0, 0, 0, 0, false, true).is_none());
    assert!(should_moderate("hello⠀world", 0, 0, 0, 0, false, true).is_none());
}

#[test]
fn test_leading_and_trailing_flood_edge_cases() {
    // Dot at start, 500 newlines, dot at end
    let flood_both = format!(".{}.", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_both, 0, 0, 5, 0, false, true),
        Some("501 lines".to_string())
    );

    // Dot at start, 500 newlines, NO dot at end (trailing flood)
    let flood_trailing = format!(".{}", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_trailing, 0, 0, 5, 0, false, true),
        Some("501 lines".to_string())
    );

    // 500 newlines, dot at end (leading flood)
    let flood_leading = format!("{}.", "\n".repeat(500));
    assert_eq!(
        should_moderate(&flood_leading, 0, 0, 5, 0, false, true),
        Some("501 lines".to_string())
    );

    // Dot at start, 500 spaces, NO dot at end
    let spaces_trailing = format!(".{}", " ".repeat(500));
    assert_eq!(
        should_moderate(&spaces_trailing, 100, 0, 0, 0, false, true),
        Some("501 characters".to_string())
    );

    // 500 spaces, dot at end
    let spaces_leading = format!("{}.", " ".repeat(500));
    assert_eq!(
        should_moderate(&spaces_leading, 100, 0, 0, 0, false, true),
        Some("501 characters".to_string())
    );
}

#[test]
fn test_integration_with_message_filter_rules() {
    use crate::domain::moderator::message_filter::{
        ModerationAction, ModerationRule, RuleCondition, should_moderate as top_level_moderate,
    };

    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: RuleCondition::ScreenFlooding {
            max_characters: 100,
            max_words: 10,
            max_lines: 5,
            chars_per_line: 40,
            disallow_invisible_chars: false,
            disallow_empty_messages: true,
        },
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
    let rules_no_empty_ban = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: RuleCondition::ScreenFlooding {
            max_characters: 0,
            max_words: 0,
            max_lines: 0,
            chars_per_line: 0,
            disallow_invisible_chars: false,
            disallow_empty_messages: false,
        },
    }];
    assert!(top_level_moderate("   ", &rules_no_empty_ban).is_none());
}

#[test]
fn test_zero_limits_treated_as_unlimited() {
    // A message with lots of characters, words, lines and soft-wrapping:
    let large_message = "word ".repeat(100) + "\n".repeat(50).as_str() + &"a".repeat(500);

    // 1. Check with explicit positive limits to verify it indeed gets moderated:
    assert!(should_moderate(&large_message, 10, 0, 0, 0, false, true).is_some());
    assert!(should_moderate(&large_message, 0, 10, 0, 0, false, true).is_some());
    assert!(should_moderate(&large_message, 0, 0, 10, 0, false, true).is_some());

    // 2. Setting each limit individually to 0 disables it:
    assert!(should_moderate(&large_message, 0, 0, 0, 0, false, true).is_none());
    assert!(should_moderate(&large_message, 0, 0, 0, 0, false, true).is_none());
    assert!(should_moderate(&large_message, 0, 0, 0, 0, false, true).is_none());

    // 3. Setting all limits to 0 simultaneously:
    assert!(should_moderate(&large_message, 0, 0, 0, 0, false, true).is_none());

    // 4. chars_per_line = 0 disables soft wrapping:
    let line_50 = "a".repeat(50);
    assert!(should_moderate(&line_50, 0, 0, 3, 10, false, true).is_some());
    assert!(should_moderate(&line_50, 0, 0, 3, 0, false, true).is_none());

    // 5. Completely empty/invisible message is still moderated when disallow_empty_messages = true even if limits are 0:
    assert_eq!(
        should_moderate("   ", 0, 0, 0, 0, false, true),
        Some("empty message".to_string())
    );

    // 6. When disallow_empty_messages = false, empty message is NOT moderated:
    assert!(should_moderate("   ", 0, 0, 0, 0, false, false).is_none());
}

#[test]
fn test_serde_json_compatibility() {
    use crate::domain::moderator::message_filter::RuleCondition;

    // 1. Deserializing full JSON with all integer fields:
    let json_full = r#"{
        "type": "ScreenFlooding",
        "max_characters": 100,
        "max_words": 20,
        "max_lines": 5,
        "chars_per_line": 35,
        "disallow_empty_messages": true,
        "disallow_invisible_chars": false
    }"#;
    let condition: RuleCondition = serde_json::from_str(json_full).unwrap();
    match condition {
        RuleCondition::ScreenFlooding {
            max_characters,
            max_words,
            max_lines,
            chars_per_line,
            disallow_empty_messages,
            disallow_invisible_chars,
        } => {
            assert_eq!(max_characters, 100);
            assert_eq!(max_words, 20);
            assert_eq!(max_lines, 5);
            assert_eq!(chars_per_line, 35);
            assert!(disallow_empty_messages);
            assert!(!disallow_invisible_chars);
        }
        _ => panic!("wrong variant"),
    }

    // 2. Deserializing JSON with nulls:
    let json_nulls = r#"{
        "type": "ScreenFlooding",
        "max_characters": null,
        "max_words": null,
        "max_lines": null,
        "chars_per_line": null
    }"#;
    let condition: RuleCondition = serde_json::from_str(json_nulls).unwrap();
    match condition {
        RuleCondition::ScreenFlooding {
            max_characters,
            max_words,
            max_lines,
            chars_per_line,
            disallow_empty_messages,
            disallow_invisible_chars,
        } => {
            assert_eq!(max_characters, 0);
            assert_eq!(max_words, 0);
            assert_eq!(max_lines, 0);
            assert_eq!(chars_per_line, 40);
            assert!(disallow_empty_messages);
            assert!(!disallow_invisible_chars);
        }
        _ => panic!("wrong variant"),
    }

    // 3. Deserializing minimal JSON {"type": "ScreenFlooding"}:
    let json_min = r#"{"type": "ScreenFlooding"}"#;
    let condition: RuleCondition = serde_json::from_str(json_min).unwrap();
    match condition {
        RuleCondition::ScreenFlooding {
            max_characters,
            max_words,
            max_lines,
            chars_per_line,
            disallow_empty_messages,
            disallow_invisible_chars,
        } => {
            assert_eq!(max_characters, 0);
            assert_eq!(max_words, 0);
            assert_eq!(max_lines, 0);
            assert_eq!(chars_per_line, 40);
            assert!(disallow_empty_messages);
            assert!(!disallow_invisible_chars);
        }
        _ => panic!("wrong variant"),
    }

    // 4. Serialization always includes all integer and boolean fields (never omitted):
    let serialized = serde_json::to_string(&condition).unwrap();
    assert!(serialized.contains(r#""max_characters":0"#));
    assert!(serialized.contains(r#""max_words":0"#));
    assert!(serialized.contains(r#""max_lines":0"#));
    assert!(serialized.contains(r#""chars_per_line":40"#));
    assert!(serialized.contains(r#""disallow_empty_messages":true"#));
    assert!(serialized.contains(r#""disallow_invisible_chars":false"#));
}
