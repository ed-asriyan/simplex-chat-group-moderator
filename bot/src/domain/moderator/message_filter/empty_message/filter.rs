/// Characters that render as blank/invisible but are not classified as
/// whitespace by Rust's `char::is_whitespace` (e.g. zero-width spaces, word
/// joiners, byte-order marks, soft hyphens). Users can pad an otherwise-empty
/// message with these so it still contains "characters" and passes a naive
/// emptiness check, while visually the message is blank.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}' // soft hyphen
            | '\u{180E}' // Mongolian vowel separator
            | '\u{200B}' // zero width space
            | '\u{200C}' // zero width non-joiner
            | '\u{200D}' // zero width joiner
            | '\u{2060}' // word joiner
            | '\u{2061}'..='\u{2064}' // invisible operators/separator
            | '\u{FEFF}' // zero width no-break space / BOM
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation chars
    )
}

/// Returns whether the message is "empty": nothing left after trimming
/// regular whitespace and invisible/zero-width characters.
pub fn should_moderate(message: &str) -> Option<String> {
    if message
        .chars()
        .all(|c| c.is_whitespace() || is_invisible(c))
    {
        Some(String::new())
    } else {
        None
    }
}
