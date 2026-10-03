//! Which characters render as nothing. Shared by `IsBlank`, which asks
//! whether a message is made of nothing else, and
//! `ContainsInvisibleCharacters`, which asks whether it hides any.

use icu_properties::CodePointSetData;
use icu_properties::CodePointSetDataBorrowed;
use icu_properties::props::DefaultIgnorableCodePoint;
use std::sync::LazyLock;

static DEFAULT_IGNORABLE: LazyLock<CodePointSetDataBorrowed<'static>> =
    LazyLock::new(|| CodePointSetData::new::<DefaultIgnorableCodePoint>());

/// Characters that render as blank/invisible or formatting-only but are not
/// classified as whitespace by Rust's `char::is_whitespace`. Users can pad an
/// otherwise-empty message with these so it still contains "characters" and
/// passes a naive emptiness check, while visually the message is blank.
///
/// Most such characters (zero-width spaces, joiners, bidi controls, variation
/// selectors, soft hyphen, Hangul fillers, tag characters, ...) are covered by
/// Unicode's `Default_Ignorable_Code_Point` property, which the Unicode
/// Consortium maintains and extends with new Unicode versions. A small set of
/// characters render blank but are deliberately *not* default-ignorable
/// (e.g. Braille pattern blank, a real "no dots" Braille glyph) and are
/// listed explicitly below.
pub fn is_invisible(c: char) -> bool {
    // Non-whitespace control characters (excludes \t, \n, \r which are covered by is_whitespace)
    if c.is_control() && !c.is_whitespace() {
        return true;
    }

    if DEFAULT_IGNORABLE.contains(c) {
        return true;
    }

    matches!(
        c,
        '\u{2800}' // Braille pattern blank
            | '\u{180E}' // Mongolian vowel separator (excluded from Default_Ignorable since Unicode 6.3)
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation chars (not Default_Ignorable)
    )
}
