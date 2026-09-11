use super::{MAX_SEQUENCE_LENGTH, should_moderate};

#[test]
fn test_single_character_repeated() {
    assert_eq!(
        should_moderate("nooooo way", 5, 1),
        Some(r#""o" repeated 5 times in a row"#.to_string())
    );
    assert!(should_moderate("noooo way", 5, 1).is_none());
}

#[test]
fn test_multi_character_sequence_repeated() {
    assert_eq!(
        should_moderate("hahahahaha", 5, 1),
        Some(r#""ha" repeated 5 times in a row"#.to_string())
    );
    assert_eq!(
        should_moderate("xx abcabcabcabcabc yy", 5, 1),
        Some(r#""abc" repeated 5 times in a row"#.to_string())
    );
    assert!(should_moderate("abcabcabcabc", 5, 1).is_none());
}

#[test]
fn test_reports_the_full_length_of_the_run() {
    assert_eq!(
        should_moderate("!!!!!!!!!!!!", 5, 1),
        Some(r#""!" repeated 12 times in a row"#.to_string())
    );
}

#[test]
fn test_min_length_ignores_shorter_sequences() {
    // Doubled letters are ordinary words, not spam.
    assert!(should_moderate("keep", 2, 1).is_some());
    assert!(should_moderate("keep", 2, 2).is_none());
    assert!(should_moderate("hello bookkeeper", 2, 2).is_none());
    assert_eq!(
        should_moderate("buy now buy now ", 2, 2),
        Some(r#""buy now " repeated 2 times in a row"#.to_string())
    );
}

#[test]
fn test_long_run_of_a_short_sequence_still_counts_as_a_longer_one() {
    // "aaaaaaaaaa" is "aa" five times, just as `(.{2,})\1{4,}` would see it.
    assert!(should_moderate("aaaaa", 5, 2).is_none());
    assert_eq!(
        should_moderate("aaaaaaaaaa", 5, 2),
        Some(r#""aa" repeated 5 times in a row"#.to_string())
    );
}

#[test]
fn test_sequences_longer_than_the_maximum_are_not_detected() {
    let at_max = "x".repeat(MAX_SEQUENCE_LENGTH as usize - 1) + "y";
    assert!(should_moderate(&at_max.repeat(2), 2, MAX_SEQUENCE_LENGTH).is_some());

    let too_long = "x".repeat(MAX_SEQUENCE_LENGTH as usize) + "y";
    assert!(should_moderate(&too_long.repeat(2), 2, MAX_SEQUENCE_LENGTH).is_none());
}

#[test]
fn test_is_case_insensitive() {
    assert_eq!(
        should_moderate("HaHAhahAHa", 5, 2),
        Some(r#""Ha" repeated 5 times in a row"#.to_string())
    );
    assert!(should_moderate("АааАа", 5, 1).is_some());
}

#[test]
fn test_counts_multi_code_point_emoji_as_a_whole() {
    // "❤️" is U+2764 U+FE0F and "👍🏻" is U+1F44D U+1F3FB.
    assert!(should_moderate(&"❤️".repeat(5), 5, 1).is_some());
    assert!(should_moderate(&"❤️".repeat(4), 5, 1).is_none());
    assert!(should_moderate(&"👍🏻".repeat(5), 5, 1).is_some());
    assert!(should_moderate("😂😂😂😂😂", 5, 1).is_some());
}

#[test]
fn test_whitespace_counts_as_characters() {
    assert!(should_moderate("a     b", 5, 1).is_some());
    assert!(should_moderate("a\n\n\n\n\nb", 5, 1).is_some());
}

#[test]
fn test_ordinary_messages_pass() {
    for message in [
        "",
        "hi",
        "Hello, how are you doing today?",
        "Привет! Как дела?",
        "see you at 10:00",
    ] {
        assert!(should_moderate(message, 3, 2).is_none(), "{message:?}");
    }
}

#[test]
fn test_invalid_settings_never_match() {
    assert!(should_moderate("aaaaaaaa", 1, 1).is_none());
    assert!(should_moderate("aaaaaaaa", 0, 1).is_none());
    assert!(should_moderate("aaaaaaaa", 5, 0).is_none());
}

#[test]
fn test_huge_repeat_count_does_not_overflow() {
    assert!(should_moderate("aaaaaaaa", u32::MAX, 1).is_none());
}
