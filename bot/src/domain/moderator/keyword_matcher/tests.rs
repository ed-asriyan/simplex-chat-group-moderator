use super::*;

fn kws(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn matches_exact_word() {
    assert!(contains_keyword("hello world", &kws(&["hello"])));
}

#[test]
fn matches_case_insensitively() {
    assert!(contains_keyword("Hello World", &kws(&["world"])));
    assert!(contains_keyword("HELLO", &kws(&["hello"])));
}

#[test]
fn does_not_match_substring() {
    assert!(!contains_keyword("classic", &kws(&["ass"])));
    assert!(!contains_keyword("therapist", &kws(&["rapist"])));
}

#[test]
fn ignores_surrounding_punctuation() {
    assert!(contains_keyword("hello, world!", &kws(&["world"])));
    assert!(contains_keyword("(spam).", &kws(&["spam"])));
    assert!(contains_keyword("end-of-line", &kws(&["line"])));
}

#[test]
fn matches_multi_word_keyword() {
    assert!(contains_keyword(
        "buy cheap pills now",
        &kws(&["cheap pills"])
    ));
    assert!(!contains_keyword("cheap and pills", &kws(&["cheap pills"])));
}

#[test]
fn skips_empty_and_whitespace_keywords() {
    assert!(!contains_keyword("hello", &kws(&["", "   "])));
}

#[test]
fn empty_text_does_not_match() {
    assert!(!contains_keyword("", &kws(&["hello"])));
    assert!(!contains_keyword("!!!", &kws(&["hello"])));
}

#[test]
fn any_matching_keyword_triggers() {
    assert!(contains_keyword(
        "the quick brown fox",
        &kws(&["lazy", "fox"])
    ));
}
