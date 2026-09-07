use super::should_moderate;

#[test]
fn test_empty_string() {
    assert_eq!(should_moderate(""), Some(String::new()));
}

#[test]
fn test_whitespace_only() {
    assert_eq!(should_moderate("   "), Some(String::new()));
    assert_eq!(should_moderate("\n\n\t \r\n"), Some(String::new()));
}

#[test]
fn test_many_newlines_only() {
    let message = "\n".repeat(10_000);
    assert_eq!(should_moderate(&message), Some(String::new()));
}

#[test]
fn test_invisible_characters_only() {
    // Zero-width spaces / joiners / BOM interspersed with regular whitespace.
    assert_eq!(
        should_moderate("\u{200B}\u{200B}\u{200B}"),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("  \u{FEFF}\u{200C}\n\u{200D}\u{2060} \u{00AD} "),
        Some(String::new())
    );
}

#[test]
fn test_braille_blank_pattern() {
    // U+2800 Braille Pattern Blank (often used to forge blank messages / floods)
    assert_eq!(should_moderate("⠀"), Some(String::new()));
    assert_eq!(should_moderate("⠀\n⠀\n\n⠀"), Some(String::new()));
}

#[test]
fn test_bidi_and_variation_selectors_only() {
    // Bidi overrides/isolates & variation selectors without visible content
    assert_eq!(
        should_moderate("\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}"),
        Some(String::new())
    );
    assert_eq!(
        should_moderate("\u{FE00}\u{FE0F}\u{E0100}"),
        Some(String::new())
    );
}

#[test]
fn test_control_characters_only() {
    // Non-whitespace control characters like NUL, BEL, DEL, C1 controls
    assert_eq!(
        should_moderate("\u{0000}\u{0007}\u{001B}\u{007F}\u{0080}\u{009F}"),
        Some(String::new())
    );
}

#[test]
fn test_non_empty_message() {
    assert!(should_moderate("hello").is_none());
    assert!(should_moderate("  hello  ").is_none());
}

#[test]
fn test_non_empty_with_invisible_characters() {
    assert!(should_moderate("\u{200B}hello\u{200B}").is_none());
}
