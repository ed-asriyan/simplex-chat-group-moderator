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
fn test_non_empty_message() {
    assert!(should_moderate("hello").is_none());
    assert!(should_moderate("  hello  ").is_none());
}

#[test]
fn test_non_empty_with_invisible_characters() {
    assert!(should_moderate("\u{200B}hello\u{200B}").is_none());
}
