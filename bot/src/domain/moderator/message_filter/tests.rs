use super::filter::*;

fn kws(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// trivial / sanity
// ---------------------------------------------------------------------------

#[test]
fn empty_text_is_allowed() {
    assert!(should_moderate("", &kws(&["spam"])).is_none());
}

#[test]
fn empty_keyword_list_is_allowed() {
    assert!(should_moderate("hello world", &[]).is_none());
}

#[test]
fn whitespace_only_keyword_is_ignored() {
    assert!(should_moderate("hello", &kws(&["", "   ", "\t"])).is_none());
}

#[test]
fn text_without_letters_is_allowed() {
    assert!(should_moderate("!!! ??? ...", &kws(&["spam"])).is_none());
}

#[test]
fn non_matching_text_is_allowed() {
    assert!(should_moderate("the quick brown fox", &kws(&["spam", "ads"])).is_none());
}

// ---------------------------------------------------------------------------
// basic EN matching
// ---------------------------------------------------------------------------

#[test]
fn matches_exact_word_en() {
    assert!(should_moderate("hello world", &kws(&["hello"])).is_some());
}

#[test]
fn returns_matched_keyword() {
    assert_eq!(
        should_moderate("hello world", &kws(&["hello"])).as_deref(),
        Some("hello")
    );
    assert_eq!(
        should_moderate("the quick brown fox", &kws(&["lazy", "fox"])).as_deref(),
        Some("fox")
    );
}

#[test]
fn is_case_insensitive_en() {
    assert!(should_moderate("Hello WORLD", &kws(&["world"])).is_some());
    assert!(should_moderate("HELLO", &kws(&["hello"])).is_some());
    assert!(should_moderate("hello", &kws(&["HELLO"])).is_some());
}

#[test]
fn ignores_surrounding_punctuation_en() {
    assert!(should_moderate("hello, world!", &kws(&["world"])).is_some());
    assert!(should_moderate("(spam).", &kws(&["spam"])).is_some());
    assert!(should_moderate("end-of-line", &kws(&["line"])).is_some());
}

#[test]
fn matches_multi_word_keyword_en() {
    assert!(should_moderate("buy cheap pills now", &kws(&["cheap pills"])).is_some());
    assert!(should_moderate("cheap and pills", &kws(&["cheap pills"])).is_none());
}

#[test]
fn any_matching_keyword_triggers_en() {
    assert!(should_moderate("the quick brown fox", &kws(&["lazy", "fox"])).is_some());
}

#[test]
fn does_not_match_substring_inside_ordinary_word_en() {
    // "ass" must not match inside "classic" — no separators / look-alikes,
    // so the merge-heuristic does not apply.
    assert!(should_moderate("classic music", &kws(&["ass"])).is_none());
    assert!(should_moderate("therapist", &kws(&["rapist"])).is_none());
}

// ---------------------------------------------------------------------------
// doubled letters must be preserved (regression: keyword "butt" vs word "but")
// ---------------------------------------------------------------------------

#[test]
fn double_letter_keyword_does_not_match_single_letter_word() {
    // Real-world false positive: keyword "butt" must NOT flag the ordinary
    // word "but". A keyword with a doubled letter requires *at least* that
    // many repeats in the text, so "but" (single `t`) is not a match.
    assert!(
        should_moderate(
            "I cannot scroll back but when I click nothing happens",
            &kws(&["butt"])
        )
        .is_none()
    );
    assert!(should_moderate("but", &kws(&["butt"])).is_none());
    // keyword "pass" (double `s`) must not match the bare word "pas".
    assert!(should_moderate("pas de deux", &kws(&["pass"])).is_none());
    // keyword "hello" (double `l`) must not match "helo".
    assert!(should_moderate("say helo", &kws(&["hello"])).is_none());
}

#[test]
fn double_letter_keyword_still_matches_real_word() {
    assert!(should_moderate("what a butt", &kws(&["butt"])).is_some());
    assert!(should_moderate("nice butt!", &kws(&["butt"])).is_some());
}

#[test]
fn double_letter_keyword_survives_flooding() {
    // Flooding a *non-doubled* letter still collapses onto the keyword.
    assert!(should_moderate("buuutt", &kws(&["butt"])).is_some());
    assert!(should_moderate("greeeat butt", &kws(&["butt"])).is_some());
}

#[test]
fn doubled_letter_keyword_matches_flooded_double_en() {
    // Real-world false negative: keyword "boob" (with a doubled `o`) must
    // still catch heavily flooded forms where the doubled letter is flooded
    // and/or extra trailing letters are appended.
    assert!(should_moderate("boob", &kws(&["boob"])).is_some());
    assert!(should_moderate("booob", &kws(&["boob"])).is_some());
    assert!(should_moderate("booooobs everywhere", &kws(&["boob"])).is_some());
    assert!(should_moderate("booooob", &kws(&["boob"])).is_some());
    assert!(should_moderate("boooooobb", &kws(&["boob"])).is_some());
}

#[test]
fn doubled_letter_keyword_still_respects_word_boundaries() {
    // "boob" must not fire on unrelated words that merely share letters.
    assert!(should_moderate("the job is done", &kws(&["boob"])).is_none());
    assert!(should_moderate("a big bob", &kws(&["boob"])).is_none());
}

// ---------------------------------------------------------------------------
// doubled letters in the *keyword* must not collapse onto innocent words
// (regression: keyword "ass"/"a55" matched "as still" in normal prose)
// ---------------------------------------------------------------------------

#[test]
fn ass_keyword_does_not_match_as_plus_following_word() {
    // Real-world false positive: "a55"/"ass" flagged the sentence below
    // because full flood-collapsing folded "ass" → "as" and the merge
    // heuristic glued "as" onto a neighbouring "s".
    let msg = "So I was testing the self destruct feature on my alt account and \
               it doesn't actually delete the account? I can still see it \
               unconnected to the various groups it was in and I can still \
               message it as well.";
    assert!(should_moderate(msg, &kws(&["a55"])).is_none());
    assert!(should_moderate(msg, &kws(&["ass"])).is_none());
}

#[test]
fn ass_keyword_does_not_match_plain_as() {
    assert!(should_moderate("as well as before", &kws(&["ass"])).is_none());
    assert!(should_moderate("as soon as possible", &kws(&["ass"])).is_none());
}

#[test]
fn ass_keyword_still_matches_real_word() {
    assert!(should_moderate("don't be an ass", &kws(&["ass"])).is_some());
    assert!(should_moderate("such an a55", &kws(&["ass"])).is_some());
}

// ---------------------------------------------------------------------------
// basic RU matching
// ---------------------------------------------------------------------------

#[test]
fn matches_exact_word_ru() {
    assert!(should_moderate("привет мир", &kws(&["привет"])).is_some());
}

#[test]
fn is_case_insensitive_ru() {
    assert!(should_moderate("ПРИВЕТ", &kws(&["привет"])).is_some());
    assert!(should_moderate("Привет", &kws(&["ПРИВЕТ"])).is_some());
}

#[test]
fn ignores_surrounding_punctuation_ru() {
    assert!(should_moderate("привет, мир!", &kws(&["мир"])).is_some());
    assert!(should_moderate("(спам).", &kws(&["спам"])).is_some());
}

#[test]
fn matches_multi_word_keyword_ru() {
    assert!(should_moderate("купи дешёвые таблетки сейчас", &kws(&["дешёвые таблетки"])).is_some());
    assert!(should_moderate("дешёвые и таблетки", &kws(&["дешёвые таблетки"])).is_none());
}

#[test]
fn yo_is_equivalent_to_ye_ru() {
    assert!(should_moderate("ёлка", &kws(&["елка"])).is_some());
    assert!(should_moderate("елка", &kws(&["ёлка"])).is_some());
}

#[test]
fn does_not_match_substring_inside_ordinary_word_ru() {
    // "рак" must not match inside "барак"
    assert!(should_moderate("барак обама", &kws(&["рак"])).is_none());
}

// ---------------------------------------------------------------------------
// bypass: separators inserted between letters
// ---------------------------------------------------------------------------

#[test]
fn detects_space_separated_letters_en() {
    assert!(should_moderate("watch out: s p a m incoming", &kws(&["spam"])).is_some());
}

#[test]
fn detects_dot_separated_letters_en() {
    assert!(should_moderate("s.p.a.m here", &kws(&["spam"])).is_some());
}

#[test]
fn detects_dash_separated_letters_en() {
    assert!(should_moderate("s-p-a-m here", &kws(&["spam"])).is_some());
}

#[test]
fn detects_underscore_separated_letters_en() {
    assert!(should_moderate("s_p_a_m here", &kws(&["spam"])).is_some());
}

#[test]
fn detects_mixed_separator_letters_en() {
    assert!(should_moderate("s. p-a_m!", &kws(&["spam"])).is_some());
}

#[test]
fn detects_space_separated_letters_ru() {
    assert!(should_moderate("с п а м прямо тут", &kws(&["спам"])).is_some());
}

#[test]
fn detects_dot_separated_letters_ru() {
    assert!(should_moderate("с.п.а.м здесь", &kws(&["спам"])).is_some());
}

#[test]
fn detects_dash_separated_letters_ru() {
    assert!(should_moderate("с-п-а-м здесь", &kws(&["спам"])).is_some());
}

// ---------------------------------------------------------------------------
// bypass: repeated characters
// ---------------------------------------------------------------------------

#[test]
fn detects_repeated_letters_en() {
    assert!(should_moderate("spaaaaam everywhere", &kws(&["spam"])).is_some());
    assert!(should_moderate("ssssspppaaammm", &kws(&["spam"])).is_some());
}

#[test]
fn detects_repeated_letters_ru() {
    assert!(should_moderate("спаааам везде", &kws(&["спам"])).is_some());
}

// ---------------------------------------------------------------------------
// bypass: leet substitutions
// ---------------------------------------------------------------------------

#[test]
fn detects_leet_digits() {
    assert!(should_moderate("5p4m incoming", &kws(&["spam"])).is_some());
    assert!(should_moderate("5P@M", &kws(&["spam"])).is_some());
    assert!(should_moderate("$pam", &kws(&["spam"])).is_some());
}

#[test]
fn detects_leet_with_repeats_and_separators() {
    assert!(should_moderate("5-p-4-a-m", &kws(&["spam"])).is_some());
    assert!(should_moderate("5 p 4 4 m", &kws(&["spam"])).is_some());
}

#[test]
fn keyword_written_in_leet_also_works() {
    // even if the user stores the keyword in leet, normalisation makes it
    // equivalent to the plain form.
    assert!(should_moderate("spam here", &kws(&["5p4m"])).is_some());
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
    assert!(should_moderate("сообщение: спам", &kws(&["спам"])).is_some());
}

#[test]
fn detects_mixed_cyrillic_latin_in_word() {
    // "scаm" with cyrillic `а` — keyword "scam" in pure latin.
    assert!(should_moderate("this is a scаm offer", &kws(&["scam"])).is_some());
}

#[test]
fn detects_latin_lookalikes_for_cyrillic_keyword() {
    // word "сос" (Russian for SOS) typed with latin `c`, `o`, `c`
    assert!(should_moderate("помогите cоc!", &kws(&["сос"])).is_some());
}

// ---------------------------------------------------------------------------
// bypass: combined tricks
// ---------------------------------------------------------------------------

#[test]
fn detects_combined_separators_repeats_and_leet() {
    assert!(should_moderate("5--p..aaa  M!!!", &kws(&["spam"])).is_some());
}

#[test]
fn detects_combined_tricks_ru() {
    assert!(should_moderate("с-п-аааа-м!!!", &kws(&["спам"])).is_some());
}

#[test]
fn multi_word_keyword_survives_separators() {
    assert!(
        should_moderate(
            "купи д.е.ш.ё.в.ы.е таблетки сейчас",
            &kws(&["дешёвые таблетки"])
        )
        .is_some()
    );
}

// ---------------------------------------------------------------------------
// negative cases that must NOT be flagged
// ---------------------------------------------------------------------------

#[test]
fn unrelated_text_with_short_words_is_allowed() {
    // Bunch of legit short words should not accidentally merge into a banned
    // word.
    assert!(should_moderate("I am at my own home", &kws(&["spam"])).is_none());
}

#[test]
fn partial_overlap_does_not_match() {
    // keyword "spamster" should NOT match text "spam" alone
    assert!(should_moderate("spam", &kws(&["spamster"])).is_none());
}

#[test]
fn keyword_longer_than_text_does_not_match() {
    assert!(should_moderate("hi", &kws(&["hello there friend"])).is_none());
}

// ---------------------------------------------------------------------------
// english plural handling
// ---------------------------------------------------------------------------

#[test]
fn keyword_matches_plural_in_text_en() {
    assert!(should_moderate("buy spams now", &kws(&["spam"])).is_some());
    assert!(should_moderate("look at the boxes", &kws(&["box"])).is_some());
    assert!(should_moderate("two parties tonight", &kws(&["party"])).is_some());
}

#[test]
fn plural_keyword_matches_singular_in_text_en() {
    assert!(should_moderate("this is spam", &kws(&["spams"])).is_some());
    assert!(should_moderate("one box only", &kws(&["boxes"])).is_some());
    assert!(should_moderate("the party", &kws(&["parties"])).is_some());
}

#[test]
fn plural_handles_es_after_sibilants_en() {
    assert!(should_moderate("the dishes are clean", &kws(&["dish"])).is_some());
    assert!(should_moderate("two churches", &kws(&["church"])).is_some());
    assert!(should_moderate("many quizzes", &kws(&["quiz"])).is_some());
}

#[test]
fn plural_survives_bypass_tricks_en() {
    // separators + leet + a trailing plural `s`
    assert!(should_moderate("5-p-a-m-s incoming", &kws(&["spam"])).is_some());
    assert!(should_moderate("s p a m s", &kws(&["spam"])).is_some());
    assert!(should_moderate("spaaaams", &kws(&["spam"])).is_some());
}

#[test]
fn plural_stripping_does_not_mangle_short_words_en() {
    // "bus", "gas", "ads" are too short / end in `s` already → must NOT
    // be stripped down to "bu"/"ga"/"ad" and accidentally match.
    assert!(should_moderate("the bus is late", &kws(&["bu"])).is_none());
    assert!(should_moderate("no gas left", &kws(&["ga"])).is_none());
    assert!(should_moderate("see the ads", &kws(&["ad"])).is_none());
}

#[test]
fn plural_stripping_does_not_affect_ru() {
    // Russian text uses cyrillic letters, so the EN plural rule must not
    // touch it: keyword "спам" still only matches the bare word.
    assert!(should_moderate("это спам", &kws(&["спам"])).is_some());
    // and a non-matching cyrillic word stays non-matching.
    assert!(should_moderate("барак обама", &kws(&["рак"])).is_none());
}
