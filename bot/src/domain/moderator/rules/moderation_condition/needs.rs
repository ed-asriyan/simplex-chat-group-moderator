//! What a condition needs the bot to record about the messages it sees, so
//! that it can answer later: how much each author and the whole group sent
//! recently, how many of an author's messages were moderated, and which
//! messages the group saw last.
//!
//! The vocabulary is the bot's counters, not the conditions: a condition says
//! which counters it reads and for how long, and `bookkeeping` records into
//! them once per message for every condition of every rule together. Two
//! conditions reading one counter therefore never count a message twice.

#[cfg(test)]
mod tests;

/// The counters and the history a condition reads. `None` means "not read";
/// every `Some` holds a non-zero window or count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    /// How many messages the author sent, over this many minutes.
    pub author_messages: Option<u32>,
    /// How many characters the author wrote, over this many minutes.
    pub author_characters: Option<u32>,
    /// How many screen lines the author's messages took.
    pub author_lines: Option<LineWindow>,
    /// How many of the author's messages were moderated, over this many
    /// minutes.
    pub author_moderations: Option<u32>,
    /// How many messages the group received, over this many minutes.
    pub group_messages: Option<u32>,
    /// How many characters the group's members wrote, over this many minutes.
    pub group_characters: Option<u32>,
    /// How many screen lines the group's messages took.
    pub group_lines: Option<LineWindow>,
    /// How many of the group's latest messages to keep.
    pub history: Option<u32>,
}

/// A line counter's window, and the width a line wraps at when a message is
/// counted into it (0: no wrapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineWindow {
    pub time_window_minutes: u32,
    pub chars_per_line: u32,
}

/// A window or a count of 0 asks for nothing.
fn nonzero(value: u32) -> Option<u32> {
    (value > 0).then_some(value)
}

fn lines(time_window_minutes: u32, chars_per_line: u32) -> Option<LineWindow> {
    nonzero(time_window_minutes).map(|time_window_minutes| LineWindow {
        time_window_minutes,
        chars_per_line,
    })
}

impl Needs {
    pub fn author_messages(time_window_minutes: u32) -> Self {
        Self {
            author_messages: nonzero(time_window_minutes),
            ..Self::default()
        }
    }

    pub fn author_characters(time_window_minutes: u32) -> Self {
        Self {
            author_characters: nonzero(time_window_minutes),
            ..Self::default()
        }
    }

    pub fn author_lines(time_window_minutes: u32, chars_per_line: u32) -> Self {
        Self {
            author_lines: lines(time_window_minutes, chars_per_line),
            ..Self::default()
        }
    }

    pub fn author_moderations(time_window_minutes: u32) -> Self {
        Self {
            author_moderations: nonzero(time_window_minutes),
            ..Self::default()
        }
    }

    pub fn group_messages(time_window_minutes: u32) -> Self {
        Self {
            group_messages: nonzero(time_window_minutes),
            ..Self::default()
        }
    }

    pub fn group_characters(time_window_minutes: u32) -> Self {
        Self {
            group_characters: nonzero(time_window_minutes),
            ..Self::default()
        }
    }

    pub fn group_lines(time_window_minutes: u32, chars_per_line: u32) -> Self {
        Self {
            group_lines: lines(time_window_minutes, chars_per_line),
            ..Self::default()
        }
    }

    pub fn history(messages: u32) -> Self {
        Self {
            history: nonzero(messages),
            ..Self::default()
        }
    }

    /// What `self` and `other` need together: the longest window and the
    /// largest history either asks for, and the widest wrap width.
    pub fn merge(self, other: Self) -> Self {
        Self {
            author_messages: longest(self.author_messages, other.author_messages),
            author_characters: longest(self.author_characters, other.author_characters),
            author_lines: merge_lines(self.author_lines, other.author_lines),
            author_moderations: longest(self.author_moderations, other.author_moderations),
            group_messages: longest(self.group_messages, other.group_messages),
            group_characters: longest(self.group_characters, other.group_characters),
            group_lines: merge_lines(self.group_lines, other.group_lines),
            history: longest(self.history, other.history),
        }
    }
}

fn longest(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// One counter serves the whole group, so a message has one line count even
/// when two conditions disagree about the width. Taking the widest makes that
/// count the one the mildest condition expects: deleting a message the owner
/// did not ask to delete is worse than missing a flood. 0 means "no wrapping",
/// which is wider than any width.
fn merge_lines(a: Option<LineWindow>, b: Option<LineWindow>) -> Option<LineWindow> {
    match (a, b) {
        (Some(a), Some(b)) => Some(LineWindow {
            time_window_minutes: a.time_window_minutes.max(b.time_window_minutes),
            chars_per_line: if a.chars_per_line == 0 || b.chars_per_line == 0 {
                0
            } else {
                a.chars_per_line.max(b.chars_per_line)
            },
        }),
        (a, b) => a.or(b),
    }
}
