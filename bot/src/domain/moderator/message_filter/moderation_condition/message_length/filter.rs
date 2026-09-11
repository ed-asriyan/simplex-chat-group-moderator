//! Measuring how long a message is, in characters, words or screen lines.
//!
//! Validation rejects a zero maximum on save, since it would store a rule that
//! can never match; each check still treats 0 as "never matches" so a bad
//! stored value is inert rather than matching every message.

// Split text by any standard Unicode newline sequence (LF, CRLF, CR, VT, FF, NEL, LS, PS).
fn split_lines(message: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = message.char_indices().peekable();

    while let Some((idx, c)) = chars.next() {
        match c {
            '\r' => {
                lines.push(&message[start..idx]);
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                }
                start = chars
                    .peek()
                    .map(|&(next_idx, _)| next_idx)
                    .unwrap_or(message.len());
            }
            '\n' | '\x0B' | '\x0C' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                lines.push(&message[start..idx]);
                start = chars
                    .peek()
                    .map(|&(next_idx, _)| next_idx)
                    .unwrap_or(message.len());
            }
            _ => {}
        }
    }
    lines.push(&message[start..]);
    lines
}

fn count_effective_lines(message: &str, chars_per_line: u32) -> usize {
    let lines = split_lines(message);
    let wrap_width = if chars_per_line > 0 {
        Some(chars_per_line as usize)
    } else {
        None
    };

    let mut total_lines = 0;
    for line in lines {
        let char_count = line.chars().count();
        match wrap_width {
            Some(w) => {
                let wrapped = if char_count == 0 {
                    1
                } else {
                    char_count.div_ceil(w)
                };
                total_lines += wrapped;
            }
            None => {
                total_lines += 1;
            }
        }
    }

    total_lines
}

/// Matches a message with more than `max_characters` characters. Whitespace
/// and line breaks count.
pub fn should_moderate_characters(message: &str, max_characters: u32) -> Option<String> {
    if max_characters == 0 {
        return None;
    }
    let count = message.chars().count();
    (count > max_characters as usize).then(|| format!("{count} characters"))
}

/// Matches a message with more than `max_words` whitespace-separated words.
pub fn should_moderate_words(message: &str, max_words: u32) -> Option<String> {
    if max_words == 0 {
        return None;
    }
    let count = message.split_whitespace().count();
    (count > max_words as usize).then(|| format!("{count} words"))
}

/// Matches a message that takes more than `max_lines` lines on screen. A line
/// longer than `chars_per_line` characters counts as several lines, as it
/// would when soft-wrapped on a narrow screen; 0 disables the wrapping.
pub fn should_moderate_lines(message: &str, max_lines: u32, chars_per_line: u32) -> Option<String> {
    if max_lines == 0 {
        return None;
    }
    let count = count_effective_lines(message, chars_per_line);
    (count > max_lines as usize).then(|| format!("{count} lines"))
}
