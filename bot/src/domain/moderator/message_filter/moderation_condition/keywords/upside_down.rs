//! Decoder for "upside-down" (flipped) text, the obfuscation produced by the
//! online *flip text* generators: `heil hitler` becomes `ɹǝlʇıɥ lıǝɥ`.
//!
//! Two things make this different from the look-alike folding that
//! [`super::filter::canonicalize`] does character by character:
//!
//! 1. **The order is reversed too** — not just the letters within a word, but
//!    the words themselves. So the text has to be reversed as a whole before
//!    it can be read; reversing token by token would find `hitler` but never
//!    the phrase `heil hitler`.
//! 2. **The alphabet reuses ordinary ASCII letters with a different meaning**:
//!    `u` stands for `n`, `n` for `u`, `q` for `b`, `d` for `p`. Folding those
//!    in `canonicalize` would wreck every ordinary message (`but` → `bnt`), so
//!    the mapping may only be applied to a separate, reversed view of the text.
//!
//! Because reversing plain text on its own would invent words that are not
//! there (`god`/`dog`, `war`/`raw`, `loop`/`pool`), [`decode`] only produces a
//! view when the text actually carries the marks of a flip: characters from
//! [`is_marker`], which a flip generator emits and a human writing prose does
//! not. Letters shared with Turkish (`ı`) or Spanish (`¡`, `¿`) are decoded but
//! deliberately do not count as marks.

/// How many flip marks the text must carry before it is read upside down.
/// One is not enough: a single IPA symbol quoted in a linguistics discussion
/// should not turn the whole message around.
const MIN_MARKERS: usize = 2;

/// Returns `text` read upside down — reversed, with the flipped alphabet
/// mapped back — or `None` if the text does not look flipped at all.
/// Characters outside the flipped alphabet are kept as they are.
pub fn decode(text: &str) -> Option<String> {
    if text.chars().filter(|&c| is_marker(c)).count() < MIN_MARKERS {
        return None;
    }
    Some(text.chars().rev().map(|c| unflip(c).unwrap_or(c)).collect())
}

/// Whether `c` is a character a flip generator produces that is vanishingly
/// unlikely to be typed on purpose. Deliberately excludes everything the
/// flipped alphabet shares with ordinary writing: the ASCII letters and digits
/// it reuses (`u`, `n`, `q`, `d`, `9`, `6`, ...), Turkish `ı`, Spanish `¡`/`¿`,
/// the quotation and bracket swaps, and `˙`/`'`.
fn is_marker(c: char) -> bool {
    matches!(
        c,
        // lower case
        'ɐ' | 'ɒ' | 'ɔ' | 'ǝ' | 'ɟ' | 'ƃ' | 'ɥ' | 'ᴉ' | 'ɾ' | 'ʞ' | 'ʃ' | 'ʅ' | 'ꞁ' | 'ן'
        | 'ɯ' | 'ɹ' | 'ʇ' | 'ʌ' | 'ʍ' | 'ʎ'
        // upper case
        | '∀' | 'ᗺ' | 'Ɔ' | 'ᗡ' | 'Ǝ' | 'Ⅎ' | '⅁' | 'ſ' | '˥' | 'Ԁ' | 'ᴚ' | '⊥' | '∩' | 'Λ' | '⅄'
        // digits
        | 'Ɩ' | 'ᄅ' | 'Ɛ' | 'ㄣ' | 'ϛ' | 'ㄥ'
        // punctuation
        | '⅋' | '‾'
    )
}

/// Maps one flipped character back to the character it stands for, or `None`
/// if it is not part of the flipped alphabet. Characters that flip onto
/// themselves (`o`, `s`, `x`, `z`, `H`, `I`, `N`, `O`, `S`, `X`, `Z`, `0`,
/// `8`) need no entry — they are left untouched by the fallback.
fn unflip(c: char) -> Option<char> {
    Some(match c {
        // lower case
        'ɐ' | 'ɒ' => 'a',
        'q' => 'b',
        'ɔ' => 'c',
        'p' => 'd',
        'ǝ' => 'e',
        'ɟ' => 'f',
        'ƃ' => 'g',
        'ɥ' => 'h',
        'ı' | 'ᴉ' => 'i',
        'ɾ' => 'j',
        'ʞ' => 'k',
        'ʃ' | 'ʅ' | 'ꞁ' | 'ן' => 'l',
        'ɯ' => 'm',
        'u' => 'n',
        'd' => 'p',
        'b' => 'q',
        'ɹ' => 'r',
        'ʇ' => 't',
        'n' => 'u',
        'ʌ' => 'v',
        'ʍ' => 'w',
        'ʎ' => 'y',
        // upper case
        '∀' => 'A',
        'ᗺ' => 'B',
        'Ɔ' => 'C',
        'ᗡ' => 'D',
        'Ǝ' => 'E',
        'Ⅎ' => 'F',
        '⅁' => 'G',
        'ſ' => 'J',
        '˥' => 'L',
        'W' => 'M',
        'Ԁ' => 'P',
        'Ò' | 'Ό' => 'Q',
        'ᴚ' => 'R',
        '⊥' => 'T',
        '∩' => 'U',
        'Λ' => 'V',
        'M' => 'W',
        '⅄' => 'Y',
        // digits
        'Ɩ' => '1',
        'ᄅ' => '2',
        'Ɛ' => '3',
        'ㄣ' => '4',
        'ϛ' => '5',
        '9' => '6',
        'ㄥ' => '7',
        '6' => '9',
        // punctuation
        '¡' => '!',
        '¿' => '?',
        '˙' => '.',
        '\'' => ',',
        '„' => '"',
        '؛' => ';',
        '⅋' => '&',
        '‾' => '_',
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        _ => return None,
    })
}
