/// Characters that render as blank/invisible or formatting-only but are not
/// classified as whitespace by Rust's `char::is_whitespace` (e.g. zero-width spaces,
/// word joiners, byte-order marks, soft hyphens, Braille blank pattern, bidi controls,
/// and variation selectors). Users can pad an otherwise-empty message with these so it
/// still contains "characters" and passes a naive emptiness check, while visually the
/// message is blank.
fn is_invisible(c: char) -> bool {
    if c.is_control() {
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
