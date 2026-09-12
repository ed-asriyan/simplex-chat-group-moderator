//! What a rule does to a message (and its author) once its condition matches.
//!
//! Each variant is one indivisible thing the bot can do, so a rule carries a
//! *list* of them rather than one action with nested "and also..." settings.
//! The list is a set: `action_planner` decides which of the listed (and of the
//! other matched rules') actions actually run, and in what order. Only the shape
//! of a single action lives here.

use crate::domain::moderator::ports::Err;
use serde::{Deserialize, Serialize};

/// The longest an author may be held as an observer before the bot restores
/// them: 30 days. It caps timed restrictions only — `0` means "indefinitely",
/// which is a different thing from a very long timer and stays allowed.
pub const MAX_OBSERVER_DURATION_MINUTES: u32 = 30 * 24 * 60;

/// One action to perform when a moderation rule triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationAction {
    #[default]
    ModerateMessage,
    SetAuthorObserver {
        /// How long the author stays an observer before the bot restores them to
        /// member. `0` restricts them indefinitely, with no restore scheduled.
        #[serde(default)]
        duration_minutes: u32,
    },
    KickAuthor {
        /// Whether to delete every message the author has sent in the group, as
        /// opposed to leaving their history in place.
        #[serde(default)]
        delete_all_messages: bool,
    },
}

impl ModerationAction {
    /// Checks and canonicalizes one action's own parameters, the way
    /// `ModerationCondition::normalize_and_validate` does for one condition.
    ///
    /// Only a single action is in scope here: what a *list* of actions means
    /// (duplicates, one action covering another, execution order) belongs to
    /// `action_planner`.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        match self {
            ModerationAction::SetAuthorObserver { duration_minutes } => {
                if *duration_minutes > MAX_OBSERVER_DURATION_MINUTES {
                    return Err(format!(
                        "Observer duration too long: {duration_minutes} minutes, maximum is {MAX_OBSERVER_DURATION_MINUTES} (30 days)"
                    )
                    .into());
                }
                Ok(())
            }
            // Listed rather than covered by a catch-all arm, so a new action
            // cannot skip validation by accident.
            ModerationAction::ModerateMessage | ModerationAction::KickAuthor { .. } => Ok(()),
        }
    }
}
