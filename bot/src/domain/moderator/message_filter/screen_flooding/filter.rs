/// Characters that render as blank/invisible or formatting-only but are not
/// classified as whitespace by Rust's `char::is_whitespace` (e.g. zero-width spaces,
/// word joiners, byte-order marks, soft hyphens, Braille blank pattern, bidi controls,
/// and variation selectors). Users can pad an otherwise-empty message with these so it
/// still contains "characters" and passes a naive emptiness check, while visually the
/// message is blank.
pub fn is_invisible(c: char) -> bool {
    // Non-whitespace control characters (excludes \t, \n, \r which are covered by is_whitespace)
    if c.is_control() && !c.is_whitespace() {
        return true;
    }

    matches!(
        c,
        '\u{00AD}' // soft hyphen
            | '\u{180E}' // Mongolian vowel separator
            | '\u{200B}' // zero width space
            | '\u{200C}' // zero width non-joiner
            | '\u{200D}' // zero width joiner
            | '\u{200E}'..='\u{200F}' // left-to-right / right-to-left mark
            | '\u{202A}'..='\u{202E}' // bidi embedding / override / pop
            | '\u{2060}' // word joiner
            | '\u{2061}'..='\u{2064}' // invisible operators/separator
            | '\u{2066}'..='\u{2069}' // bidi isolates
            | '\u{2800}' // Braille pattern blank
            | '\u{FE00}'..='\u{FE0F}' // variation selectors 1..16
            | '\u{FEFF}' // zero width no-break space / BOM
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation chars
            | '\u{E0001}' | '\u{E0020}'..='\u{E007F}' // language tag characters
            | '\u{E0100}'..='\u{E01EF}' // variation selectors supplement
    )
}

fn is_blank(c: char) -> bool {
    c.is_whitespace() || is_invisible(c)
}

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

pub fn should_moderate(
    message: &str,
    max_characters: u32,
    max_words: u32,
    max_lines: u32,
    chars_per_line: u32,
    disallow_invisible_chars: bool,
    disallow_empty_messages: bool,
) -> Option<String> {
    // 1. Fully empty / whitespace / invisible message check
    if disallow_empty_messages && message.chars().all(is_blank) {
        return Some("empty message".to_string());
    }

    // 2. Disallow any invisible / zero-width characters in the message
    if disallow_invisible_chars && message.chars().any(is_invisible) {
        return Some("invisible characters".to_string());
    }

    // 3. Max characters limit (0 to disable)
    if max_characters > 0 {
        let count = message.chars().count();
        if count > max_characters as usize {
            return Some(format!("{count} characters"));
        }
    }

    // 4. Max words limit (0 to disable)
    if max_words > 0 {
        let count = message.split_whitespace().count();
        if count > max_words as usize {
            return Some(format!("{count} words"));
        }
    }

    // 5. Max lines limit (0 to disable, soft wrap controlled by chars_per_line)
    if max_lines > 0 {
        let count = count_effective_lines(message, chars_per_line);
        if count > max_lines as usize {
            return Some(format!("{count} lines"));
        }
    }

    None
}
