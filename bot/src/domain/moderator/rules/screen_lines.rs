//! How many lines a message takes on screen.
//!
//! Two things measure a message this way — `ExceedsMaxLines`, which looks at one
//! message, and the line rate limit, which counts every message an author sends
//! into a window — and the second counts from the use case, not from a
//! condition. So the measurement sits here, beside the entities that use it,
//! rather than inside either of them: the same reason `action_planner` is not
//! inside `moderation_action`.

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

/// How many lines the message takes on screen, with every line longer than
/// `chars_per_line` characters counted as several; 0 disables the wrapping.
pub fn count_effective_lines(message: &str, chars_per_line: u32) -> usize {
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

#[cfg(test)]
mod tests;
