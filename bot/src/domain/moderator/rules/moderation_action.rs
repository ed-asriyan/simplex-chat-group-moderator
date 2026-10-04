//! What a rule does to a message (and its author) once its condition matches.
//!
//! Each action is one indivisible thing the bot can do, so a rule carries a
//! *list* of them rather than one action with nested "and also..." settings.
//! Each is a type of its own, in a module of its own, implementing [`Action`]:
//! its parameters and how they are checked, what it achieves ([`Effect`]),
//! where it goes in the execution order, and how it is carried out. This module
//! only lists them (`actions!`) and carries out a planned list.
//!
//! The list is a set: `action_planner` decides which of the listed (and of the
//! other matched rules') actions actually run, and in what order, from what
//! each one says about itself.

#[cfg(test)]
mod fakes;
#[cfg(test)]
mod tests;

use crate::domain::moderator::ports::{Err, GroupMessage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use context::ActionPorts;
pub(in crate::domain::moderator::rules) use context::{ActionContext, ActionReport};
pub(in crate::domain::moderator::rules) use effect::{Effect, Silence};

mod context;
mod effect;

/// What every action is to the code that plans and carries out a rule's
/// actions: a set of operations, the same for all of them. Nothing outside an
/// action's own module knows what one does or how.
#[async_trait]
pub(in crate::domain::moderator::rules) trait Action:
    Send + Sync
{
    /// Checks and canonicalizes the action's own parameters when an owner
    /// saves the rule. What a *list* of actions means (duplicates, one action
    /// covering another, execution order) belongs to `action_planner`.
    fn normalize_and_validate(&mut self) -> Result<(), Err>;

    /// What carrying the action out achieves. One action makes another
    /// redundant when it achieves at least as much on every count.
    fn effect(&self) -> Effect;

    /// Where the action goes in the safe execution order; lower runs first.
    /// Restricting the author comes first, then moderating the message, and
    /// kicking the author last — once they are out of the group, acting on
    /// them or on their message may no longer be possible. The order
    /// deliberately does not come from the order the owner listed the actions
    /// in: a rule's action list is a set.
    fn execution_rank(&self) -> u8;

    /// Carries the action out on the message that matched. An error here
    /// stops what is left of the moderation; a failure of the bot's own
    /// records goes into the report instead, so that it does not.
    async fn execute(&self, ctx: &ActionContext<'_>) -> Result<ActionReport, Err>;
}

/// Declares every action — one line each, `Variant => module` — and builds
/// [`ModerationAction`] from them.
macro_rules! actions {
    ($($variant:ident => $module:ident,)*) => {
        $(mod $module;)*

        /// Every action's own type, for code that builds or reads one.
        pub mod actions {
            $(pub use super::$module::$variant;)*
        }

        /// One action to perform when a moderation rule triggers.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "type")]
        pub enum ModerationAction {
            $($variant($module::$variant),)*
        }

        impl ModerationAction {
            fn as_leaf(&self) -> &dyn Action {
                match self {
                    $(Self::$variant(action) => action,)*
                }
            }

            fn as_leaf_mut(&mut self) -> &mut dyn Action {
                match self {
                    $(Self::$variant(action) => action,)*
                }
            }

            /// The serde tag: the variant's name, as the rules JSON, the editor
            /// and the database spell it.
            pub fn type_name(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => stringify!($variant),)*
                }
            }
        }
    };
}

actions! {
    ModerateMessage => moderate_message,
    SetAuthorObserver => set_author_observer,
    KickAuthor => kick_author,
}

impl ModerationAction {
    /// Checks and canonicalizes one action's own parameters, the way
    /// `ModerationCondition::normalize_and_validate` does for one condition.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        self.as_leaf_mut().normalize_and_validate()
    }

    pub(in crate::domain::moderator::rules) fn effect(&self) -> Effect {
        self.as_leaf().effect()
    }

    pub(in crate::domain::moderator::rules) fn execution_rank(&self) -> u8 {
        self.as_leaf().execution_rank()
    }
}

/// What carrying out a matched rule's actions came to.
pub struct ActionOutcome {
    /// Whether the message that matched is gone from the chat.
    pub message_deleted: bool,
    /// The first failure of the bot's own records, reported once everything
    /// else has run.
    pub bookkeeping_error: Option<Err>,
}

/// Carries out the planned `actions`, in the order given, on the message that
/// matched. The first action that fails stops the rest.
pub async fn execute_actions(
    group_message: &GroupMessage,
    actions: &[ModerationAction],
    ports: &ActionPorts<'_>,
) -> Result<ActionOutcome, Err> {
    let ctx = ActionContext {
        group_message,
        ports,
    };
    let mut bookkeeping_error = None;
    for action in actions {
        let report = action.as_leaf().execute(&ctx).await?;
        bookkeeping_error = bookkeeping_error.or(report.bookkeeping_error);
    }
    Ok(ActionOutcome {
        message_deleted: actions.iter().any(|action| action.effect().message_deleted),
        bookkeeping_error,
    })
}
