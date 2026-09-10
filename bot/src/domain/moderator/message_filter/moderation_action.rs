//! What a rule does to a message (and its author) once its condition matches.
//!
//! Only the shape of an action lives here. Deciding which of several matched
//! actions actually run — and in what order — is `action_planner`'s job.

use serde::{Deserialize, Serialize};

/// Action to perform on the author's messages when kicking an author.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteAuthorMessages {
    None,
    #[default]
    TriggeredMessage,
    AllMessages,
}

/// Action to perform on the author's messages when setting an author as observer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteObserverMessages {
    None,
    #[default]
    TriggeredMessage,
}

/// Action to perform when a moderation rule triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationAction {
    #[default]
    ModerateMessage,
    KickAuthor {
        #[serde(default)]
        delete_messages: DeleteAuthorMessages,
    },
    SetAuthorObserver {
        #[serde(default)]
        delete_message: DeleteObserverMessages,
    },
}
