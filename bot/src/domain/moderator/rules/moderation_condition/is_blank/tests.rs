use super::*;

fn blank() -> Option<String> {
    Some("empty message".to_string())
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
