//! Single-purpose message filter for the moderator bounded context.
//!
//! Given an incoming message text and a list of blocked keywords, decides
//! whether the message should be moderated (deleted).
//!
//! The filter is intentionally bypass-resistant. It transparently sees through:
//!
//! - case differences (`Spam`, `SPAM`, `sPaM`);
//! - leet substitutions (`5p4m`, `$pam`, `@ss`);
//! - cyrillic look-alikes mixed into latin and vice versa (`sраm` with
//!   cyrillic `р`+`а`, `спам` vs `spam`);
//! - separators inserted between letters (`s p a m`, `s.p.a.m`, `s-p-a-m`,
//!   `с_п_а_м`);
//! - repeated characters (`spaaaaam`, `goooal`).
//!
//! Word boundaries are still respected for "ordinary" text — `"ass"` does
//! **not** match inside `"classic"` (no separators/look-alikes are present
//! so the merging heuristic is not applied to that token).

/// Returns `true` iff `text` should be moderated (deleted) given the list
/// of `blocked_keywords`. Empty / whitespace-only keywords are ignored.
pub fn should_moderate(text: &str, blocked_keywords: &[String]) -> bool {
    let tokens = normalize_and_tokenize(text);
    if tokens.is_empty() {
        return false;
    }
    let merged = merge_short_runs(&tokens);

    blocked_keywords.iter().any(|kw| {
        let needle = normalize_and_tokenize(kw);
        if needle.is_empty() {
            return false;
        }
        contains_subsequence(&tokens, &needle) || contains_subsequence(&merged, &needle)
    })
}

// ---------------------------------------------------------------------------
// internals
// ---------------------------------------------------------------------------

/// Lowercase + canonicalise + split on non-alphanumerics + collapse runs of
/// repeated characters within each token.
fn normalize_and_tokenize(s: &str) -> Vec<String> {
    let normalized: String = s
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(canonicalize)
        .collect();

    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(collapse_repeats)
        .collect()
}

/// Canonicalise a single (already lower-cased) character.
///
/// - leet digits / symbols collapse to the latin letter they resemble;
/// - cyrillic letters that share a glyph with a latin one collapse to the
///   latin counterpart (so `с` ≡ `c`, `р` ≡ `p`, ...). Cyrillic-only letters
///   (`б`, `г`, `д`, `ж`, ...) are left untouched, which is exactly what we
///   want: keywords typed in Russian still match Russian text, and Russian
///   text "hidden" inside latin look-alikes (and vice versa) is unmasked.
fn canonicalize(c: char) -> char {
    match c {
        // leet
        '0' => 'o',
        '1' => 'i',
        '3' => 'e',
        '4' => 'a',
        '5' => 's',
        '7' => 't',
        '8' => 'b',
        '9' => 'g',
        '@' => 'a',
        '$' => 's',
        // cyrillic → latin look-alikes
        'а' => 'a',
        'в' => 'b',
        'е' => 'e',
        'ё' => 'e',
        'к' => 'k',
        'м' => 'm',
        'н' => 'h',
        'о' => 'o',
        'р' => 'p',
        'с' => 'c',
        'т' => 't',
        'у' => 'y',
        'х' => 'x',
        other => other,
    }
}

/// Collapse runs of the same character down to a single occurrence.
/// `"spaaaam"` → `"spam"`, `"goooooal"` → `"goal"`.
fn collapse_repeats(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if Some(c) != prev {
            out.push(c);
            prev = Some(c);
        }
    }
    out
}

/// Merge consecutive short tokens (≤ 2 characters) into a single token.
/// `["s", "p", "a", "m"]` → `["spam"]`. Long tokens are left untouched and
/// break the run. This is what lets us see through `"s.p.a.m"` while still
/// keeping `"classic"` as one token (so `"ass"` will not match inside it).
fn merge_short_runs(tokens: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut buf = String::new();
    for t in tokens {
        if t.chars().count() <= 2 {
            buf.push_str(t);
        } else {
            if !buf.is_empty() {
                out.push(collapse_repeats(&std::mem::take(&mut buf)));
            }
            out.push(t.clone());
        }
    }
    if !buf.is_empty() {
        out.push(collapse_repeats(&buf));
    }
    out
}

/// Returns true iff `needle` appears as a contiguous run of tokens inside
/// `haystack` (whole-token equality, no substring).
fn contains_subsequence(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}
