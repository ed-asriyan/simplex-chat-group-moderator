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
//! - repeated characters (`spaaaaam`, `goooal`);
//! - simple English plural forms (`spams`, `boxes`, `parties`) — both the
//!   text and the keyword are stripped of trailing `-s`/`-es`/`-ies`, so a
//!   keyword `spam` matches `spams` and a keyword `spams` matches `spam`.
//!
//! Word boundaries are still respected for "ordinary" text — `"ass"` does
//! **not** match inside `"classic"` (no separators/look-alikes are present
//! so the merging heuristic is not applied to that token).

/// Returns the first matched keyword iff `text` should be moderated (deleted)
/// given the list of `blocked_keywords`, or `None` if no keyword matches.
/// Empty / whitespace-only keywords are ignored.
pub fn should_moderate(text: &str, blocked_keywords: &[String]) -> Option<String> {
    let tokens = normalize_and_tokenize(text);
    if tokens.is_empty() {
        return None;
    }
    let merged = merge_short_runs(&tokens);

    blocked_keywords.iter().find_map(|kw| {
        let needle = normalize_and_tokenize(kw);
        if needle.is_empty() {
            return None;
        }
        if contains_subsequence(&tokens, &needle) || contains_subsequence(&merged, &needle) {
            Some(kw.clone())
        } else {
            None
        }
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
        .map(|t| depluralize_en(&t))
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

/// Strip common English plural suffixes (`-ies` → `-y`, `-es`, `-s`) from a
/// token whose characters are already lower-cased and canonicalised.
///
/// Only ASCII-letter tokens are touched, so Cyrillic words pass through
/// unchanged. The stem is required to be at least 3 characters long so that
/// short legitimate words ending in `s` (e.g. `bus`, `gas`) are not mangled.
fn depluralize_en(s: &str) -> String {
    // Only consider tokens made entirely of ASCII letters; anything else
    // (digits, cyrillic, mixed) is left alone.
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_lowercase()) {
        return s.to_string();
    }

    // `-ies` → `-y` (parties → party, cities → city). Needs a stem of ≥ 2
    // letters before the `ies` so we don't turn `ties` into `ty`.
    if s.len() > 4 && s.ends_with("ies") {
        let mut stem = s[..s.len() - 3].to_string();
        stem.push('y');
        return stem;
    }

    // `-es` after a sibilant cluster (boxes, buses, dishes, churches,
    // quizzes). Requires a stem of ≥ 3 letters.
    if s.len() > 4 && s.ends_with("es") {
        let stem = &s[..s.len() - 2];
        let ends_in_sibilant = stem.ends_with('s')
            || stem.ends_with('x')
            || stem.ends_with('z')
            || stem.ends_with("sh")
            || stem.ends_with("ch");
        if ends_in_sibilant {
            return stem.to_string();
        }
    }

    // Plain `-s`. Requires a stem of ≥ 3 letters and a stem that does not
    // itself already end in `s` (avoid touching `bus`, `gas`, `class`).
    if s.len() > 3 && s.ends_with('s') {
        let stem = &s[..s.len() - 1];
        if !stem.ends_with('s') {
            return stem.to_string();
        }
    }

    s.to_string()
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
                let collapsed = collapse_repeats(&std::mem::take(&mut buf));
                out.push(depluralize_en(&collapsed));
            }
            out.push(t.clone());
        }
    }
    if !buf.is_empty() {
        let collapsed = collapse_repeats(&buf);
        out.push(depluralize_en(&collapsed));
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
