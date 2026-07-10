//! Single-purpose message filter for the moderator bounded context.
//!
//! Given an incoming message text and a list of blocked keywords, decides
//! whether the message should be moderated (deleted).
//!
//! The filter is intentionally bypass-resistant. It transparently sees through:
//!
//! - case differences (`Spam`, `SPAM`, `sPaM`);
//! - leet substitutions (`5p4m`, `$pam`, `5p@m`);
//! - `@`-prefixed mentions (`@crawlerbot` matches the keyword `crawlerbot`) —
//!   a `@` at the start of a word is treated as a mention sigil and dropped,
//!   while a `@` inside a word is still read as the letter `a`;
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
//!   keyword `spam` matches `spams` and a keyword `spams` matches `spam`;
//! - common derivational / inflectional suffixes (`sex`→`sexy`,
//!   `sexual`→`sexually`/`sexuality`, `masturbate`→`masturbation`/
//!   `masturbating`, `dog`→`doggy`/`doggie`) — a token is reduced to a base
//!   form by stripping one recognised suffix and undoing the consonant
//!   doubling such a suffix triggers (`titty`→`titt`→`tit`), so morphological
//!   variants of a keyword still match;
//! - compound words written joined or split (`blowjob` ⇄ `blow job`,
//!   `doggystyle` ⇄ `doggy style`) — for longer keywords the tokens are glued
//!   and fuzzily normalised so a window of text tokens matches regardless of
//!   where word boundaries fall.
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
        if needle_present(&tokens, &merged, &needle) || compound_present(&tokens, &needle) {

            return Some(kw.clone());
        }
        None
    })
}

/// True iff `needle` appears in either the plain or the short-run-merged view
/// of the text tokens.
fn needle_present(tokens: &[String], merged: &[String], needle: &[String]) -> bool {
    contains_subsequence(tokens, needle) || contains_subsequence(merged, needle)
}

/// Generic join/split compound matcher. Glues the keyword tokens into one
/// string, fuzzily normalises it (collapse repeats, `ie`→`y`, drop the soft
/// `y`/`i` vowels that mark spelling/diminutive variants) and looks for a
/// contiguous window of text tokens whose glued+normalised form is identical.
///
/// This is what lets `blowjob` match `blow job` and `dog style` match
/// `doggystyle`, irrespective of where the word boundaries land. It only fires
/// for keywords whose normalised form is reasonably long (≥ 5 chars), so short
/// keywords like `ass`/`cum`/`tit` keep strict per-token word boundaries and
/// never collapse onto innocent neighbours.
fn compound_present(haystack: &[String], needle: &[String]) -> bool {
    let gn = fuzzy_compound(&needle.concat());
    if gn.chars().count() < 5 {
        return false;
    }
    let gn_len = gn.chars().count();
    for start in 0..haystack.len() {
        let mut glued = String::new();
        for tok in &haystack[start..] {
            glued.push_str(tok);
            let gw = fuzzy_compound(&glued);
            if gw == gn {
                return true;
            }
            if gw.chars().count() > gn_len {
                break;
            }
        }
    }
    false
}

/// Collapse all repeated characters to one, fold `ie`→`y`, then drop the soft
/// vowels `y`/`i`. Used only by [`compound_present`] to reconcile spelling and
/// diminutive variants of compound words (`dog`/`doggy`/`doggie`).
fn fuzzy_compound(s: &str) -> String {
    let mut collapsed = String::with_capacity(s.len());
    let mut last: Option<char> = None;
    for c in s.chars() {
        if Some(c) != last {
            collapsed.push(c);
            last = Some(c);
        }
    }
    collapsed
        .replace("ie", "y")
        .chars()
        .filter(|&c| c != 'y' && c != 'i')
        .collect()
}

// ---------------------------------------------------------------------------
// internals
// ---------------------------------------------------------------------------

/// Lowercase + canonicalise + split on non-alphanumerics. Repeated characters
/// are *not* collapsed here — their run lengths are preserved so that matching
/// can be run-length-aware (a flooded `"booooob"` still carries enough `o`s to
/// satisfy a doubled-letter keyword like `"boob"`).
fn normalize_and_tokenize(s: &str) -> Vec<String> {
    // Lower-case first so positional checks below see the final characters.
    let lowered: Vec<char> = s.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut normalized = String::with_capacity(lowered.len());
    for (i, &c) in lowered.iter().enumerate() {
        if c == '@' {
            // A `@` that opens a word is a mention sigil (`@crawlerbot`): emit a
            // separator so it splits off rather than becoming the letter `a`.
            // A `@` inside a word stays leet-`a` (`5p@m` -> `spam`).
            let prev_is_alnum = i > 0 && lowered[i - 1].is_alphanumeric();
            if prev_is_alnum {
                normalized.push('a');
            } else {
                normalized.push(' ');
            }
        } else {
            normalized.push(canonicalize(c));
        }
    }

    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
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

/// Produce candidate singular forms for an (already lower-cased / canonicalised)
/// token, used for run-length-aware matching. The token itself is always a
/// candidate; recognised English plural suffixes contribute additional stems.
///
/// Only ASCII-letter tokens get extra candidates, so Cyrillic words pass
/// through unchanged. Generating *several* stems (rather than committing to a
/// single guess) lets a keyword like `boob` match both `boobs` (`-s`) and
/// `boobies` (`-y` → `-ies`, whose bare stem is also offered).
fn singular_candidates(s: &str) -> Vec<String> {
    let mut out = vec![s.to_string()];

    // Only consider tokens made entirely of ASCII letters; anything else
    // (digits, cyrillic, mixed) is left alone.
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_lowercase()) {
        return out;
    }

    let collapsed = collapse_floods(s);
    if collapsed != s {
        // If the token is flooded, also consider the stems of its collapsed form.
        // This allows suffix stripping to work on flooded suffixes (e.g. `nessss` -> `ines`).
        let mut stems_collapsed = singular_candidates(&collapsed);
        out.append(&mut stems_collapsed);
    }

    // Collect every base form reachable by stripping a single recognised
    // suffix. Each stem also contributes its "un-doubled" variant, undoing the
    // consonant doubling a suffix triggers (`titty`→`titt`→`tit`,
    // `doggy`→`dogg`→`dog`). The doubling is only undone on stems produced by
    // stripping a suffix — never on the bare token — so `butt`/`pass`/`hello`
    // keep their doubled letters and a keyword `butt` still does not match
    // `but`.
    let mut stems: Vec<String> = Vec::new();

    // `-ies`: offer both the `-y` form (parties → party, cities → city) and
    // the bare stem (boobies → boob, cookies → cook).
    if s.len() > 4 && s.ends_with("ies") {
        let stem = &s[..s.len() - 3];
        stems.push(format!("{stem}y"));
        if stem.len() >= 3 {
            stems.push(stem.to_string());
        }
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
                stems.push(format!("{base}z"));
            } else {
                stems.push(stem.to_string());
            }
        }
    }

    // Plain `-s`. Requires a stem of ≥ 3 letters and a stem that does not
    // itself already end in `s` (avoid touching `bus`, `gas`, `class`).
    if s.len() > 3 && s.ends_with('s') {
        let stem = &s[..s.len() - 1];
        if !stem.ends_with('s') {
            stems.push(stem.to_string());
        }
    }

    // Derivational / diminutive / inflectional suffixes that reduce a word to a
    // shorter base form. Listed longest-first so the most specific one wins.
    for suffix in [
        "ations", "ation", "iests", "iest", "ings", "ing", "ions", "ion", "iness", "ness", "ines",
        "nes", "ities", "ity", "iers", "ier", "eds", "ed", "ers", "er", "lys", "ly", "ies", "y",
        "e",
    ] {
        if let Some(stem) = s.strip_suffix(suffix)
            && stem.len() >= 3
        {
            stems.push(stem.to_string());
        }
    }

    for stem in stems {
        if let Some(undoubled) = undouble_final(&stem) {
            out.push(undoubled);
        }
        out.push(stem);
    }
    out.sort();
    out.dedup();
    out
}

/// If `s` ends in a doubled consonant (`titt`, `dogg`, `embass`), return the
/// form with a single final consonant (`tit`, `dog`, `embas`); otherwise
/// `None`. Vowels are never un-doubled (so `boob`/`bee` are untouched).
fn undouble_final(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let n = b.len();
    if n >= 2 && b[n - 1] == b[n - 2] && is_consonant(b[n - 1]) {
        Some(s[..n - 1].to_string())
    } else {
        None
    }
}

/// True for ASCII consonants (an ASCII letter that is not a vowel).
fn is_consonant(c: u8) -> bool {
    c.is_ascii_alphabetic() && !matches!(c, b'a' | b'e' | b'i' | b'o' | b'u')
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
                out.push(std::mem::take(&mut buf));
            }
            out.push(t.clone());
        }
    }
    if !buf.is_empty() {
        out.push(buf);
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

/// True iff `a`'s run-length encoding "covers" `b`'s: same sequence of distinct
/// letters and, for each letter, `a` repeats it *at least* as many times as
/// `b`. So `"booooob"` covers `"boob"`, but `"bob"` does not, and `"but"` does
/// not cover `"butt"`.
fn rle_covers(a: &str, b: &str) -> bool {
    let a = rle(a);
    let b = rle(b);
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(&(ac, an), &(bc, bn))| ac == bc && an >= bn)
}

/// True iff `text_tok` matches `kw_tok`, tolerating flooded (repeated) letters
/// and English plural forms on either side. Each token contributes a few
/// candidate singular stems; a match on any text-candidate / keyword-candidate
/// pair counts.
fn token_matches(text_tok: &str, kw_tok: &str) -> bool {
    let text_cands = singular_candidates(text_tok);
    let kw_cands = singular_candidates(kw_tok);
    text_cands
        .iter()
        .any(|tc| kw_cands.iter().any(|kc| rle_covers(tc, kc)))
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
