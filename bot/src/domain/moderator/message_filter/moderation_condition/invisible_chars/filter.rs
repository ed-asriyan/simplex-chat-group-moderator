use icu_properties::CodePointSetData;
use icu_properties::CodePointSetDataBorrowed;
use icu_properties::props::{DefaultIgnorableCodePoint, Emoji};
use std::sync::LazyLock;

static DEFAULT_IGNORABLE: LazyLock<CodePointSetDataBorrowed<'static>> =
    LazyLock::new(|| CodePointSetData::new::<DefaultIgnorableCodePoint>());
static EMOJI: LazyLock<CodePointSetDataBorrowed<'static>> =
    LazyLock::new(|| CodePointSetData::new::<Emoji>());

const ZERO_WIDTH_JOINER: char = '\u{200D}';

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
fn is_invisible(c: char) -> bool {
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

fn is_blank(c: char) -> bool {
    c.is_whitespace() || is_invisible(c)
}

/// Matches a message that consists only of whitespace, line breaks and
/// invisible characters — including one with no characters at all.
pub fn should_moderate_blank(message: &str) -> Option<String> {
    message
        .chars()
        .all(is_blank)
        .then(|| "empty message".to_string())
}

/// Matches a message containing even a single invisible character, however
/// much visible text surrounds it.
///
/// A ZERO WIDTH JOINER sandwiched between two emoji is a legitimate emoji ZWJ
/// sequence (e.g. "family" = man + ZWJ + woman + ZWJ + girl), not an attempt to
/// hide content, so it does not count.
pub fn should_moderate_invisible(message: &str) -> Option<String> {
    let chars: Vec<char> = message.chars().collect();
    chars
        .iter()
        .enumerate()
        .any(|(i, &c)| {
            if c == ZERO_WIDTH_JOINER {
                let prev_is_emoji = i > 0 && EMOJI.contains(chars[i - 1]);
                let next_is_emoji = chars.get(i + 1).is_some_and(|&n| EMOJI.contains(n));
                if prev_is_emoji && next_is_emoji {
                    return false;
                }
            }
            is_invisible(c)
        })
        .then(|| "invisible characters".to_string())
}
