use crate::domain::moderator::rules::common::invisible::is_invisible;
use icu_properties::CodePointSetData;
use icu_properties::CodePointSetDataBorrowed;
use icu_properties::props::Emoji;
use std::sync::LazyLock;

static EMOJI: LazyLock<CodePointSetDataBorrowed<'static>> =
    LazyLock::new(|| CodePointSetData::new::<Emoji>());

const ZERO_WIDTH_JOINER: char = '\u{200D}';
const TEXT_PRESENTATION_SELECTOR: char = '\u{FE0E}';
const EMOJI_PRESENTATION_SELECTOR: char = '\u{FE0F}';
const TAG_SPEC_FIRST: char = '\u{E0020}';
const TAG_TERMINATOR: char = '\u{E007F}';
/// Longest ISO 3166-2 subdivision code an emoji tag sequence can spell.
const MAX_TAG_SPEC_LEN: usize = 6;

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

fn is_variation_selector(c: char) -> bool {
    c == TEXT_PRESENTATION_SELECTOR || c == EMOJI_PRESENTATION_SELECTOR
}

fn is_tag_char(c: char) -> bool {
    (TAG_SPEC_FIRST..=TAG_TERMINATOR).contains(&c)
}

/// Whether an emoji sequence element ends right before `i` — an emoji
/// character, optionally followed by a skin-tone modifier (itself an emoji
/// character) and/or a presentation selector.
fn element_ends_before(chars: &[char], i: usize) -> bool {
    let mut end = i;
    while end > 0 && is_variation_selector(chars[end - 1]) {
        end -= 1;
    }
    end > 0 && EMOJI.contains(chars[end - 1])
}

/// Whether the tag character at `i` belongs to an emoji tag sequence
/// (UTS #51 ED-14a): a tag base, then tag specs, then CANCEL TAG — the shape
/// of the subdivision flags 🏴󠁧󠁢󠁥󠁮󠁧󠁿 🏴󠁧󠁢󠁳󠁣󠁴󠁿 🏴󠁧󠁢󠁷󠁬󠁳󠁿.
///
/// Tag characters map one-to-one onto ASCII, so an unbounded run of them is a
/// way to hide a whole message behind one visible glyph. Only a run short
/// enough to be a region code (the longest ISO 3166-2 subdivision codes are six
/// characters) is exempt.
fn in_emoji_tag_sequence(chars: &[char], i: usize) -> bool {
    let mut start = i;
    while start > 0 && is_tag_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = i;
    while end + 1 < chars.len() && is_tag_char(chars[end + 1]) {
        end += 1;
    }

    if start == 0 || !EMOJI.contains(chars[start - 1]) {
        return false;
    }
    let Some((&terminator, spec)) = chars[start..=end].split_last() else {
        return false;
    };
    terminator == TAG_TERMINATOR
        && !spec.is_empty()
        && spec.len() <= MAX_TAG_SPEC_LEN
        && spec.iter().all(|&c| c < TAG_TERMINATOR)
}

/// Whether the invisible character at `i` is part of a well-formed emoji
/// sequence (UTS #51) rather than hidden content. Emoji are built out of
/// exactly the characters this condition looks for: ❤️ is a heart plus an
/// invisible presentation selector, 👨‍👩‍👧 is three people glued with zero
/// width joiners, 🏴󠁧󠁢󠁥󠁮󠁧󠁿 is a flag plus invisible tag characters.
fn joins_emoji_sequence(chars: &[char], i: usize) -> bool {
    let c = chars[i];

    // emoji/text presentation sequence: an emoji character, then the selector
    // that picks how it renders (ED-8/ED-9a). Keycaps 1️⃣ take this shape too.
    if is_variation_selector(c) {
        return i > 0 && EMOJI.contains(chars[i - 1]);
    }

    // emoji ZWJ sequence (ED-15): a joiner between two emoji elements, e.g.
    // "family" = man + ZWJ + woman + ZWJ + girl.
    if c == ZERO_WIDTH_JOINER {
        return element_ends_before(chars, i)
            && chars.get(i + 1).is_some_and(|&n| EMOJI.contains(n));
    }

    if is_tag_char(c) {
        return in_emoji_tag_sequence(chars, i);
    }

    false
}

/// Matches a message containing even a single invisible character, however
/// much visible text surrounds it — unless that character is part of an emoji
/// sequence, which is visible content spelled with invisible glue.
pub fn should_moderate_invisible(message: &str) -> Option<String> {
    let chars: Vec<char> = message.chars().collect();
    (0..chars.len())
        .any(|i| is_invisible(chars[i]) && !joins_emoji_sequence(&chars, i))
        .then(|| "invisible characters".to_string())
}
