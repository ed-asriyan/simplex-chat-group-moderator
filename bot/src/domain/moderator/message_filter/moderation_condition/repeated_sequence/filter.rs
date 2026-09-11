/// Longest sequence (in characters) whose consecutive repeats are detected.
///
/// Bounds the matching cost: the search is `O(MAX_SEQUENCE_LENGTH * len)`.
pub const MAX_SEQUENCE_LENGTH: u32 = 50;

/// Lowercase a character when that maps it to exactly one character, so the
/// folded text keeps the original's indices (a few characters, like 'İ',
/// lowercase to two and are left as they are).
fn fold_case(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Returns a description of the first sequence of `min_length` to
/// [`MAX_SEQUENCE_LENGTH`] characters that appears at least `min_repeats` times
/// in a row, or `None` if there is none. With `min_repeats = 5, min_length = 1`
/// that is "aaaaa", "!!!!!", "hahahahaha" or "abcabcabcabcabc". Comparison is
/// case-insensitive.
///
/// This is what `(.{min_length,50})\1{min_repeats-1,}` would express, but the
/// `regex` crate has no backreferences, and a backtracking engine would expose
/// every group to catastrophic patterns. Instead, for each period `p` a single
/// pass counts how many consecutive positions satisfy `text[i] == text[i + p]`:
/// a run of `(min_repeats - 1) * p` such positions spans exactly `min_repeats`
/// copies of a `p`-character sequence. That keeps the cost linear in the
/// message length for any settings.
///
/// Emoji built from several code points ("❤️", "👍🏻") need no special handling:
/// they are simply sequences longer than one character.
pub fn should_moderate(message: &str, min_repeats: u32, min_length: u32) -> Option<String> {
    // Validation rejects these on save; guard anyway so a bad stored value can
    // never make every message match.
    if min_repeats < 2 || min_length == 0 {
        return None;
    }
    let repeats_needed = min_repeats as usize;
    let original: Vec<char> = message.chars().collect();
    let folded: Vec<char> = original.iter().map(|&c| fold_case(c)).collect();
    let len = folded.len();

    // A sequence of length `p` needs `min_repeats * p` characters to be caught.
    let longest = (MAX_SEQUENCE_LENGTH as usize).min(len / repeats_needed);
    for period in min_length as usize..=longest {
        let run_needed = (repeats_needed - 1) * period;
        let mut run = 0;
        for i in 0..len - period {
            if folded[i] != folded[i + period] {
                run = 0;
                continue;
            }
            run += 1;
            if run == run_needed {
                let start = i + 1 - run;
                // Follow the run to its end so the reason reports the full count.
                let mut end = i + 1;
                while end + period < len && folded[end] == folded[end + period] {
                    end += 1;
                }
                let repeats = (end - start + period) / period;
                let sequence: String = original[start..start + period].iter().collect();
                return Some(format!("{sequence:?} repeated {repeats} times in a row"));
            }
        }
    }

    None
}
