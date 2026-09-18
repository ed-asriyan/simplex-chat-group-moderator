use super::count_effective_lines;

#[test]
fn test_counts_every_newline_kind() {
    assert_eq!(count_effective_lines("a\nb\r\nc\rd", 0), 4);
    assert_eq!(count_effective_lines("a\x0Bb\x0Cc\u{0085}d", 0), 4);
    assert_eq!(count_effective_lines("a\u{2028}b\u{2029}c", 0), 3);
    // A trailing break opens a line the message still occupies.
    assert_eq!(count_effective_lines("a\nb\n", 0), 3);
}

#[test]
fn test_wraps_long_lines_at_the_given_width() {
    // 25 characters at a width of 10 is three screen lines.
    assert_eq!(count_effective_lines(&"a".repeat(25), 10), 3);
    assert_eq!(count_effective_lines(&"a".repeat(25), 40), 1);
    // Each line wraps on its own, and an empty one still takes a line.
    assert_eq!(count_effective_lines("\n0123456789012345", 10), 3);
}

#[test]
fn test_zero_width_disables_wrapping() {
    assert_eq!(count_effective_lines(&"a".repeat(1000), 0), 1);
}

#[test]
fn test_empty_message_is_one_empty_line() {
    assert_eq!(count_effective_lines("", 40), 1);
    assert_eq!(count_effective_lines("", 0), 1);
}
