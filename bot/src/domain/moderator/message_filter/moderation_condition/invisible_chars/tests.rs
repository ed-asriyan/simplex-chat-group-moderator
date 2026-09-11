use super::{should_moderate_blank, should_moderate_invisible};

fn blank() -> Option<String> {
    Some("empty message".to_string())
}

fn invisible() -> Option<String> {
    Some("invisible characters".to_string())
}

#[test]
fn test_empty_string_is_blank() {
    assert_eq!(should_moderate_blank(""), blank());
}

#[test]
fn test_whitespace_only_is_blank() {
    assert_eq!(should_moderate_blank("   "), blank());
    assert_eq!(should_moderate_blank("\n\n\t \r\n"), blank());
    assert_eq!(should_moderate_blank(&"\n".repeat(10_000)), blank());
}

#[test]
fn test_invisible_characters_only_is_blank() {
    assert_eq!(should_moderate_blank("\u{200B}\u{200B}\u{200B}"), blank());
    assert_eq!(
        should_moderate_blank("  \u{FEFF}\u{200C}\n\u{200D}\u{2060} \u{00AD} "),
        blank()
    );
}

#[test]
fn test_braille_blank_pattern_is_blank() {
    assert_eq!(should_moderate_blank("⠀"), blank());
    assert_eq!(should_moderate_blank("⠀\n⠀\n\n⠀"), blank());
}

#[test]
fn test_bidi_and_variation_selectors_only_is_blank() {
    assert_eq!(
        should_moderate_blank("\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}"),
        blank()
    );
    assert_eq!(should_moderate_blank("\u{FE00}\u{FE0F}\u{E0100}"), blank());
}

#[test]
fn test_hangul_filler_only_is_blank() {
    // Hangul filler characters render as blank but are letters (not whitespace/control) in Unicode.
    assert_eq!(should_moderate_blank(&"\u{3164}".repeat(709)), blank());
    assert_eq!(
        should_moderate_blank("\u{115F}\u{1160}\u{3164}\u{FFA0}"),
        blank()
    );
}

#[test]
fn test_control_characters_only_is_blank() {
    assert_eq!(
        should_moderate_blank("\u{0000}\u{0007}\u{001B}\u{007F}\u{0080}\u{009F}"),
        blank()
    );
}

#[test]
fn test_message_with_visible_text_is_not_blank() {
    assert!(should_moderate_blank("hello").is_none());
    assert!(should_moderate_blank("  hello  ").is_none());
    assert!(should_moderate_blank("\u{200B}hello\u{200B}").is_none());
    assert!(should_moderate_blank(".").is_none());
    assert!(should_moderate_blank(&format!(".{}", "\n".repeat(500))).is_none());
}

#[test]
fn test_blank_message_of_invisible_characters_also_contains_them() {
    // The two conditions are independent: a message padded with invisible
    // characters is both blank and contains invisible characters.
    assert_eq!(
        should_moderate_invisible("\u{200B}\u{200B}\u{200B}"),
        invisible()
    );
    // A blank message of plain whitespace contains none.
    assert!(should_moderate_invisible("   ").is_none());
    assert!(should_moderate_invisible("").is_none());
}

#[test]
fn test_invisible_characters_in_text() {
    // Normal text with regular whitespace (spaces, tabs, newlines, carriage returns) has none.
    assert!(should_moderate_invisible("hello world\nline 2\tindent\r\n").is_none());

    // Even a single zero-width space anywhere in otherwise normal text matches
    assert_eq!(
        should_moderate_invisible("\u{200B}hello world"),
        invisible()
    );
    assert_eq!(should_moderate_invisible("hello\u{200B}world"), invisible());
    assert_eq!(
        should_moderate_invisible("hello world\u{200B}"),
        invisible()
    );

    for (text, what) in [
        ("hello⠀world", "Braille blank U+2800"),
        ("hello\u{00AD}world", "soft hyphen"),
        ("hello\u{180E}world", "Mongolian vowel separator"),
        ("hello\u{200C}world", "zero width non-joiner"),
        ("hello\u{200D}world", "zero width joiner"),
        ("hello\u{202E}world", "bidi override"),
        ("hello\u{2060}world", "word joiner"),
        ("hello\u{2066}world", "bidi isolate"),
        ("hello\u{FE00}world", "variation selector"),
        ("hello\u{E0100}world", "variation selector supplement"),
        ("hello\u{E0020}world", "Unicode tag character"),
        ("hello\u{FFF9}world", "interlinear annotation"),
        ("hello\u{0000}world", "NUL control character"),
        ("hello\u{0007}world", "BEL control character"),
        ("hello\u{001B}world", "ESC control character"),
    ] {
        assert_eq!(should_moderate_invisible(text), invisible(), "{what}");
    }
}

#[test]
fn test_invisible_characters_false_positives() {
    // Accented Latin text using combining diacritics (NFD) is normal text, not invisible
    assert!(should_moderate_invisible("cafe\u{0301}").is_none()); // café
    assert!(should_moderate_invisible("nie\u{0300}m").is_none());

    // Vietnamese with stacked combining marks
    assert!(should_moderate_invisible("ti\u{1EBF}ng Vi\u{1EC7}t").is_none());

    // Non-Latin scripts (Cyrillic, CJK, Arabic, Hebrew, Greek) are normal visible text
    assert!(should_moderate_invisible("Привет, как дела?").is_none());
    assert!(should_moderate_invisible("こんにちは世界").is_none());
    assert!(should_moderate_invisible("你好，世界").is_none());
    assert!(should_moderate_invisible("مرحبا بالعالم").is_none());
    assert!(should_moderate_invisible("שלום עולם").is_none());
    assert!(should_moderate_invisible("Γειά σου Κόσμε").is_none());

    // Plain emoji, flag emoji (regional indicator pairs) and skin-tone modifiers are
    // visible characters, not default-ignorable
    assert!(should_moderate_invisible("🦀🎉").is_none());
    assert!(should_moderate_invisible("🇺🇸🇩🇪").is_none());
    assert!(should_moderate_invisible("👍🏽").is_none());

    // Multi-person / profession+gender emoji are built from a ZWJ (U+200D) sequence,
    // e.g. "family" = man + ZWJ + woman + ZWJ + girl + ZWJ + boy. A ZWJ sandwiched
    // between two emoji is a legitimate joiner, not hidden content, so it is exempt.
    assert!(should_moderate_invisible("👨\u{200D}👩\u{200D}👧\u{200D}👦").is_none());

    // A ZWJ that is NOT joining two emoji (e.g. hidden inside plain text) is still caught
    assert_eq!(should_moderate_invisible("hello\u{200D}world"), invisible());
    assert_eq!(should_moderate_invisible("👨\u{200D}hello"), invisible());
}
