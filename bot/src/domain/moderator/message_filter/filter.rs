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
//! - flooded characters (`spaaaaam`, `goooal`) — runs of three or more
//!   identical characters collapse to one, while legitimate doubled letters
//!   (`butt`, `pass`, `hello`) are preserved so a keyword `butt` does **not**
//!   match the ordinary word `but`;
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

/// Lowercase + canonicalise + split on non-alphanumerics. Repeated characters
/// are *not* collapsed here — their run lengths are preserved so that matching
/// can be run-length-aware (a flooded `"booooob"` still carries enough `o`s to
/// satisfy a doubled-letter keyword like `"boob"`).
fn normalize_and_tokenize(s: &str) -> Vec<String> {
    let normalized: String = s
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(canonicalize)
        .collect();

    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(depluralize_en)
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

/// Collapse *flooded* characters: a run of three or more identical characters
/// is reduced to a single occurrence, while runs of one or two characters are
/// left intact. Used only to decide whether a token is "short" enough to take
/// part in the merge heuristic (so a flooded single letter like `"aaa"` still
/// counts as a separated letter). The merged token itself keeps its original
/// run lengths for run-length-aware matching.
fn collapse_floods(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut j = i + 1;
        while j < chars.len() && chars[j] == c {
            j += 1;
        }
        let run = j - i;
        let keep = if run >= 3 { 1 } else { run };
        for _ in 0..keep {
            out.push(c);
        }
        i = j;
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
            // Words ending in `z` double it before `-es` (quiz → quizzes).
            // Undo that doubling so the stem matches the singular keyword,
            // but leave genuinely doubled stems like `glass` (glasses) alone.
            if let Some(base) = stem.strip_suffix("zz") {
                return format!("{base}z");
            }
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
///
/// "Shortness" is judged on the flood-collapsed form, so a flooded single
/// letter (`"aaa"`) still counts as a separated letter and joins the run,
/// while a long token (`"booooob"`) is pushed unchanged with its run lengths
/// intact for run-length-aware matching.
fn merge_short_runs(tokens: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut buf = String::new();
    for t in tokens {
        let collapsed = collapse_floods(t);
        if collapsed.chars().count() <= 2 {
            buf.push_str(&collapsed);
        } else {
            if !buf.is_empty() {
                out.push(depluralize_en(&std::mem::take(&mut buf)));
            }
            out.push(t.clone());
        }
    }
    if !buf.is_empty() {
        out.push(depluralize_en(&buf));
    }
    out
}

/// Run-length encode a token: `"boob"` → `[('b',1),('o',2),('b',1)]`.
fn rle(s: &str) -> Vec<(char, usize)> {
    let mut out: Vec<(char, usize)> = Vec::new();
    for c in s.chars() {
        match out.last_mut() {
            Some(last) if last.0 == c => last.1 += 1,
            _ => out.push((c, 1)),
        }
    }
    out
}

/// True iff `text_tok` matches `kw_tok` allowing flooded (repeated) letters:
/// both must have the same sequence of distinct letters and, for each letter,
/// the text must repeat it *at least* as many times as the keyword. So
/// `"booooob"` matches `"boob"`, but `"bob"` does not, and `"but"` does not
/// match `"butt"`.
fn token_matches(text_tok: &str, kw_tok: &str) -> bool {
    let a = rle(text_tok);
    let b = rle(kw_tok);
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(&(tc, tn), &(kc, kn))| tc == kc && tn >= kn)
}

/// Returns true iff `needle` appears as a contiguous run of tokens inside
/// `haystack`, matching each token with [`token_matches`] (run-length-aware,
/// no substring).
fn contains_subsequence(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| {
        w.iter()
            .zip(needle.iter())
            .all(|(h, n)| token_matches(h, n))
    })
}
