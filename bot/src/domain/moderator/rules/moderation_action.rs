//! What a rule does once its condition matches: to the message, or to the
//! member.
//!
//! Each action is one indivisible thing the bot can do, so a rule carries a
//! *list* of them rather than one action with nested "and also..." settings.
//! Each is a type of its own, in a module of its own under
//! `moderation_action/`: its parameters and how they are checked, what it
//! achieves ([`Effect`]) and where it goes in the execution order
//! ([`Action`]), and how it is carried out on a trigger's subject
//! ([`ActionOn`]). This module lists them all once (`actions!`), which builds
//! [`ModerationAction`]: an action as an owner writes it, in the rules JSON,
//! the editor and the database. A trigger takes only some of them: its module
//! lists those (`trigger_actions!`), which builds the trigger's own action
//! type, reached from a `ModerationAction` by `TryFrom`, which refuses an
//! action the trigger does not take (US-C2, US-F1).
//!
//! The list is a set: `action_planner` decides which of the listed (and of the
//! other matched rules') actions actually run, and in what order, from what
//! each one says about itself.

#[cfg(test)]
mod fakes;
#[cfg(test)]
mod tests;

use crate::domain::moderator::ports::{Err, GroupMessage, JoinAction, MemberJoin, MessageAction};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use context::ActionPorts;
pub(in crate::domain::moderator::rules) use context::{ActionContext, ActionReport};
pub(in crate::domain::moderator::rules) use effect::{Effect, Silence};

mod context;
mod effect;

/// What every action is, whatever it is carried out on: its settings, what it
/// achieves and where it goes in the order. This is all `action_planner` asks
/// of one. Nothing outside an action's own module knows what one does or how.
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
    /// Restricting the member comes first, then moderating the message, and
    /// kicking the member last — once they are out of the group, acting on
    /// them or on their message may no longer be possible. The order
    /// deliberately does not come from the order the owner listed the actions
    /// in: a rule's action list is a set.
    fn execution_rank(&self) -> u8;

    /// The action as the editor titles it, without the emoji: what an owner is
    /// told about it when a rule's trigger does not take it (US-C2).
    fn title(&self) -> &'static str;
}

/// An action carried out on `S`, the subject of a trigger: a `GroupMessage`
/// for an action on the message, any
/// [`Member`](super::common::member::Member) for one on the member. A
/// trigger lists only actions that implement this for its subject, so an
/// action in the wrong trigger does not compile.
#[async_trait]
pub(in crate::domain::moderator::rules) trait ActionOn<S: ?Sized + Sync>:
    Action
{
    /// Carries the action out on the subject that matched. An error here
    /// stops what is left of the moderation; a failure of the bot's own
    /// records goes into the report instead, so that it does not.
    async fn execute(&self, ctx: &ActionContext<'_, S>) -> Result<ActionReport, Err>;
}

/// A trigger's action type — what `trigger_actions!` builds for each trigger
/// —, as the planner and the engine use it, whichever trigger it is.
pub(in crate::domain::moderator::rules) trait TriggerAction:
    Copy + Eq + Send + Sync + Sized
{
    /// What the trigger's actions are carried out on.
    type Subject: Sync;

    fn leaf(&self) -> &dyn ActionOn<Self::Subject>;
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

        /// An action as an owner writes it: any action.
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

/// Builds a trigger's action type from the actions it takes — one line each,
/// by the action's name —, carried out on the trigger's `$subject`. It
/// converts from a [`ModerationAction`], refusing any action not listed
/// (US-C2), and back.
macro_rules! trigger_actions {
    ($name:ident on $subject:ty, $trigger:expr; $($variant:ident,)*) => {
        /// An action the trigger takes.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant($crate::domain::moderator::rules::actions::$variant),)*
        }

        impl $crate::domain::moderator::rules::moderation_action::TriggerAction for $name {
            type Subject = $subject;

            fn leaf(
                &self,
            ) -> &dyn $crate::domain::moderator::rules::moderation_action::ActionOn<$subject> {
                match self {
                    $(Self::$variant(action) => action,)*
                }
            }
        }

        impl TryFrom<$crate::domain::moderator::rules::ModerationAction> for $name {
            type Error = $crate::domain::moderator::ports::Err;

            fn try_from(
                action: $crate::domain::moderator::rules::ModerationAction,
            ) -> Result<Self, Self::Error> {
                use $crate::domain::moderator::rules::ModerationAction as Any_;
                match action {
                    $(Any_::$variant(action) => Ok(Self::$variant(action)),)*
                    #[allow(unreachable_patterns)]
                    other => Err($crate::domain::moderator::rules::trigger::not_taken(
                        other.title(),
                        $trigger,
                    )),
                }
            }
        }

        impl From<$name> for $crate::domain::moderator::rules::ModerationAction {
            fn from(action: $name) -> Self {
                match action {
                    $($name::$variant(action) => Self::$variant(action),)*
                }
            }
        }
    };
}
pub(in crate::domain::moderator::rules) use trigger_actions;

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

    /// The action as the editor titles it, without the emoji.
    pub(in crate::domain::moderator::rules) fn title(&self) -> &'static str {
        self.as_leaf().title()
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

/// Carries out the planned `actions`, in the order given, on the subject that
/// matched — a message, or someone who joined. The first action that fails
/// stops the rest.
async fn execute<A: TriggerAction>(
    subject: &A::Subject,
    actions: &[A],
    ports: &ActionPorts<'_>,
) -> Result<ActionOutcome, Err>
where
    ModerationAction: From<A>,
{
    let ctx = ActionContext { subject, ports };
    let mut bookkeeping_error = None;
    for action in actions {
        let report = action.leaf().execute(&ctx).await?;
        bookkeeping_error = bookkeeping_error.or(report.bookkeeping_error);
    }
    Ok(ActionOutcome {
        message_deleted: actions
            .iter()
            .any(|action| ModerationAction::from(*action).effect().message_deleted),
        bookkeeping_error,
    })
}

/// Carries out the planned actions of a message rule, in the order given, on
/// the message that matched (US-D1). The first action that fails stops the
/// rest.
pub async fn execute_message_actions(
    message: &GroupMessage,
    actions: &[MessageAction],
    ports: &ActionPorts<'_>,
) -> Result<ActionOutcome, Err> {
    execute(message, actions, ports).await
}

/// Carries out the planned actions of a join rule, in the order given, on the
/// person who joined (US-D6). The first action that fails stops the rest.
pub async fn execute_join_actions(
    join: &MemberJoin,
    actions: &[JoinAction],
    ports: &ActionPorts<'_>,
) -> Result<ActionOutcome, Err> {
    execute(join, actions, ports).await
}
