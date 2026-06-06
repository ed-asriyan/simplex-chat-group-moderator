use super::filter::*;

fn kws(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// trivial / sanity
// ---------------------------------------------------------------------------

#[test]
fn empty_text_is_allowed() {
    assert!(!should_moderate("", &kws(&["spam"])));
}

#[test]
fn empty_keyword_list_is_allowed() {
    assert!(!should_moderate("hello world", &[]));
}

#[test]
fn whitespace_only_keyword_is_ignored() {
    assert!(!should_moderate("hello", &kws(&["", "   ", "\t"])));
}

#[test]
fn text_without_letters_is_allowed() {
    assert!(!should_moderate("!!! ??? ...", &kws(&["spam"])));
}

#[test]
fn non_matching_text_is_allowed() {
    assert!(!should_moderate(
        "the quick brown fox",
        &kws(&["spam", "ads"])
    ));
}

// ---------------------------------------------------------------------------
// basic EN matching
// ---------------------------------------------------------------------------

#[test]
fn matches_exact_word_en() {
    assert!(should_moderate("hello world", &kws(&["hello"])));
}

#[test]
fn is_case_insensitive_en() {
    assert!(should_moderate("Hello WORLD", &kws(&["world"])));
    assert!(should_moderate("HELLO", &kws(&["hello"])));
    assert!(should_moderate("hello", &kws(&["HELLO"])));
}

#[test]
fn ignores_surrounding_punctuation_en() {
    assert!(should_moderate("hello, world!", &kws(&["world"])));
    assert!(should_moderate("(spam).", &kws(&["spam"])));
    assert!(should_moderate("end-of-line", &kws(&["line"])));
}

#[test]
fn matches_multi_word_keyword_en() {
    assert!(should_moderate(
        "buy cheap pills now",
        &kws(&["cheap pills"])
    ));
    assert!(!should_moderate("cheap and pills", &kws(&["cheap pills"])));
}

#[test]
fn any_matching_keyword_triggers_en() {
    assert!(should_moderate(
        "the quick brown fox",
        &kws(&["lazy", "fox"])
    ));
}

#[test]
fn does_not_match_substring_inside_ordinary_word_en() {
    // "ass" must not match inside "classic" — no separators / look-alikes,
    // so the merge-heuristic does not apply.
    assert!(!should_moderate("classic music", &kws(&["ass"])));
    assert!(!should_moderate("therapist", &kws(&["rapist"])));
}

// ---------------------------------------------------------------------------
// basic RU matching
// ---------------------------------------------------------------------------

#[test]
fn matches_exact_word_ru() {
    assert!(should_moderate("привет мир", &kws(&["привет"])));
}

#[test]
fn is_case_insensitive_ru() {
    assert!(should_moderate("ПРИВЕТ", &kws(&["привет"])));
    assert!(should_moderate("Привет", &kws(&["ПРИВЕТ"])));
}

#[test]
fn ignores_surrounding_punctuation_ru() {
    assert!(should_moderate("привет, мир!", &kws(&["мир"])));
    assert!(should_moderate("(спам).", &kws(&["спам"])));
}

#[test]
fn matches_multi_word_keyword_ru() {
    assert!(should_moderate(
        "купи дешёвые таблетки сейчас",
        &kws(&["дешёвые таблетки"])
    ));
    assert!(!should_moderate(
        "дешёвые и таблетки",
        &kws(&["дешёвые таблетки"])
    ));
}

#[test]
fn yo_is_equivalent_to_ye_ru() {
    assert!(should_moderate("ёлка", &kws(&["елка"])));
    assert!(should_moderate("елка", &kws(&["ёлка"])));
}

#[test]
fn does_not_match_substring_inside_ordinary_word_ru() {
    // "рак" must not match inside "барак"
    assert!(!should_moderate("барак обама", &kws(&["рак"])));
}

// ---------------------------------------------------------------------------
// bypass: separators inserted between letters
// ---------------------------------------------------------------------------

#[test]
fn detects_space_separated_letters_en() {
    assert!(should_moderate(
        "watch out: s p a m incoming",
        &kws(&["spam"])
    ));
}

#[test]
fn detects_dot_separated_letters_en() {
    assert!(should_moderate("s.p.a.m here", &kws(&["spam"])));
}

#[test]
fn detects_dash_separated_letters_en() {
    assert!(should_moderate("s-p-a-m here", &kws(&["spam"])));
}

#[test]
fn detects_underscore_separated_letters_en() {
    assert!(should_moderate("s_p_a_m here", &kws(&["spam"])));
}

#[test]
fn detects_mixed_separator_letters_en() {
    assert!(should_moderate("s. p-a_m!", &kws(&["spam"])));
}

#[test]
fn detects_space_separated_letters_ru() {
    assert!(should_moderate("с п а м прямо тут", &kws(&["спам"])));
}

#[test]
fn detects_dot_separated_letters_ru() {
    assert!(should_moderate("с.п.а.м здесь", &kws(&["спам"])));
}

#[test]
fn detects_dash_separated_letters_ru() {
    assert!(should_moderate("с-п-а-м здесь", &kws(&["спам"])));
}

// ---------------------------------------------------------------------------
// bypass: repeated characters
// ---------------------------------------------------------------------------

#[test]
fn detects_repeated_letters_en() {
    assert!(should_moderate("spaaaaam everywhere", &kws(&["spam"])));
    assert!(should_moderate("ssssspppaaammm", &kws(&["spam"])));
}

#[test]
fn detects_repeated_letters_ru() {
    assert!(should_moderate("спаааам везде", &kws(&["спам"])));
}

// ---------------------------------------------------------------------------
// bypass: leet substitutions
// ---------------------------------------------------------------------------

#[test]
fn detects_leet_digits() {
    assert!(should_moderate("5p4m incoming", &kws(&["spam"])));
    assert!(should_moderate("5P@M", &kws(&["spam"])));
    assert!(should_moderate("$pam", &kws(&["spam"])));
}

#[test]
fn detects_leet_with_repeats_and_separators() {
    assert!(should_moderate("5-p-4-a-m", &kws(&["spam"])));
    assert!(should_moderate("5 p 4 4 m", &kws(&["spam"])));
}

#[test]
fn keyword_written_in_leet_also_works() {
    // even if the user stores the keyword in leet, normalisation makes it
    // equivalent to the plain form.
    assert!(should_moderate("spam here", &kws(&["5p4m"])));
}

// ---------------------------------------------------------------------------
// bypass: cyrillic look-alikes
// ---------------------------------------------------------------------------

#[test]
fn detects_cyrillic_lookalikes_in_latin_keyword() {
    // text uses cyrillic `с` (U+0441) and `а` (U+0430) and `м` (U+043C)
    // and `р` (U+0440) to spell what looks like latin "spam":
    //   с(p)= cyrillic s-look-alike → canonical 'c'
    //   п                            → stays 'п'
    //   а                            → 'a'
    //   м                            → 'm'
    // Keyword stored as cyrillic "спам" also normalises the same way, so
    // we should still catch this when the keyword is the cyrillic form.
    assert!(should_moderate("сообщение: спам", &kws(&["спам"])));
}

#[test]
fn detects_mixed_cyrillic_latin_in_word() {
    // "scаm" with cyrillic `а` — keyword "scam" in pure latin.
    assert!(should_moderate("this is a scаm offer", &kws(&["scam"])));
}

#[test]
fn detects_latin_lookalikes_for_cyrillic_keyword() {
    // word "сос" (Russian for SOS) typed with latin `c`, `o`, `c`
    assert!(should_moderate("помогите cоc!", &kws(&["сос"])));
}

// ---------------------------------------------------------------------------
// bypass: combined tricks
// ---------------------------------------------------------------------------

#[test]
fn detects_combined_separators_repeats_and_leet() {
    assert!(should_moderate("5--p..aaa  M!!!", &kws(&["spam"])));
}

#[test]
fn detects_combined_tricks_ru() {
    assert!(should_moderate("с-п-аааа-м!!!", &kws(&["спам"])));
}

#[test]
fn multi_word_keyword_survives_separators() {
    assert!(should_moderate(
        "купи д.е.ш.ё.в.ы.е таблетки сейчас",
        &kws(&["дешёвые таблетки"])
    ));
}

// ---------------------------------------------------------------------------
// negative cases that must NOT be flagged
// ---------------------------------------------------------------------------

#[test]
fn unrelated_text_with_short_words_is_allowed() {
    // Bunch of legit short words should not accidentally merge into a banned
    // word.
    assert!(!should_moderate("I am at my own home", &kws(&["spam"])));
}

#[test]
fn partial_overlap_does_not_match() {
    // keyword "spamster" should NOT match text "spam" alone
    assert!(!should_moderate("spam", &kws(&["spamster"])));
}

#[test]
fn keyword_longer_than_text_does_not_match() {
    assert!(!should_moderate("hi", &kws(&["hello there friend"])));
}
