//! What a rule does to a message (and its author) once its condition matches.
//!
//! Each variant is one indivisible thing the bot can do, so a rule carries a
//! *list* of them rather than one action with nested "and also..." settings.
//! The list is a set: `action_planner` decides which of the listed (and of the
//! other matched rules') actions actually run, and in what order. Only the shape
//! of a single action lives here.

use serde::{Deserialize, Serialize};

/// One action to perform when a moderation rule triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationAction {
    #[default]
    ModerateMessage,
    SetAuthorObserver,
    KickAuthor {
        /// Whether to delete every message the author has sent in the group, as
        /// opposed to leaving their history in place.
        #[serde(default)]
        delete_all_messages: bool,
    },
}
