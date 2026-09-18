//! Measuring how long a message is, in characters, words or screen lines.
//!
//! Validation rejects a zero maximum on save, since it would store a rule that
//! can never match; each check still treats 0 as "never matches" so a bad
//! stored value is inert rather than matching every message.

use crate::domain::moderator::message_filter::screen_lines::count_effective_lines;

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
