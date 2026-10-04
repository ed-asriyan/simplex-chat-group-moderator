//! What an action achieves, in terms every action is described in. Coverage
//! between actions is derived from it rather than listed pair by pair, so a
//! new action is placed among the others by saying what it does.

#[cfg(test)]
mod tests;

/// What carrying out an action achieves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// The message that matched is deleted.
    pub message_deleted: bool,
    /// Every message the author sent in the group is deleted.
    pub history_deleted: bool,
    /// How long the author is kept from writing.
    pub author_silenced: Silence,
    /// The author is out of the group.
    pub author_removed: bool,
}

/// How long an author is kept from writing, weakest first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Silence {
    #[default]
    No,
    For {
        minutes: u32,
    },
    Forever,
}

impl Effect {
    /// Whether this effect achieves everything `other` does, so an action with
    /// `other`'s effect would add nothing next to one with this.
    pub fn covers(&self, other: &Effect) -> bool {
        self.message_deleted >= other.message_deleted
            && self.history_deleted >= other.history_deleted
            && self.author_silenced >= other.author_silenced
            && self.author_removed >= other.author_removed
    }
}
