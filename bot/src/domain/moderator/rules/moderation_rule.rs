//! A rule: what makes the bot check it, what it checks, and what it does on a
//! match — named by the owner and turned on or off by them (US-D1, US-D6,
//! US-G13).

use super::{ModerationAction, ModerationCondition, action_planner};
use crate::domain::moderator::ports::Err;
use serde::{Deserialize, Serialize};

/// The longest name a rule may have, in characters (US-C2).
pub const MAX_RULE_NAME_CHARS: usize = 60;

/// What makes the bot check a rule. Every condition and action says which
/// triggers it works with; a rule holding one its trigger does not take is
/// refused when the owner saves it (US-C2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// A member sends a message, or edits one (US-D1).
    #[default]
    MessagePosted,
    /// Someone joins the group as a member or an observer (US-D6).
    MemberJoined,
}

impl Trigger {
    /// The name used in the group's JSON and in the database.
    pub fn name(self) -> &'static str {
        match self {
            Self::MessagePosted => "message_posted",
            Self::MemberJoined => "member_joined",
        }
    }

    /// The trigger whose [`Self::name`] is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::MessagePosted, Self::MemberJoined]
            .into_iter()
            .find(|trigger| trigger.name() == name)
    }
}

/// A moderation rule: on its `trigger`, when its `condition` matches, the bot
/// carries out its `actions`.
///
/// The JSON is `{"name": ..., "trigger": ..., "enabled": ..., "condition":
/// {...}, "actions": [...]}`; a missing `trigger` is a message rule, a
/// missing `enabled` a rule that is on, and a missing `name` or `condition`
/// is none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationRule {
    /// What the owner calls the rule: at most [`MAX_RULE_NAME_CHARS`]
    /// characters, never blank. The notifications name a matched rule by it
    /// (US-D2, US-D6); a rule without one is reported by what matched alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub trigger: Trigger,
    /// A rule that is off is kept with its settings and sent back in the
    /// editor link, but never checked, and its actions cover nothing (US-D1,
    /// US-D6, US-G13).
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// What the rule checks. Only a rule for [`Trigger::MemberJoined`] may
    /// have none, and then matches everyone who joins (US-D6); a message rule
    /// without one is refused (US-C2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ModerationCondition>,
    pub actions: Vec<ModerationAction>,
}

fn enabled_by_default() -> bool {
    true
}

impl ModerationRule {
    /// Checks what an owner saved and rewrites it into the canonical form that
    /// is stored and evaluated (US-C2).
    ///
    /// Refuses a rule with no actions, a message rule with no condition, a
    /// condition or an action the rule's trigger does not take, and a name
    /// longer than [`MAX_RULE_NAME_CHARS`]; a blank name becomes none. Each
    /// action checks its own parameters first, then the list is normalized
    /// through the planner the same way several matched rules are merged:
    /// duplicates and actions already covered by a stronger one disappear, and
    /// what is left is ordered the way it will be executed (US-F4). The order
    /// matters — the planner compares action parameters, so they have to be
    /// settled by then. A rule that is off is checked the same: the owner can
    /// turn it on at any time without saving it again.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        if self.actions.is_empty() {
            return Err(
                "A rule has no actions. Add at least one action to it, or remove the rule.".into(),
            );
        }
        for action in &mut self.actions {
            action.normalize_and_validate()?;
        }
        self.actions = action_planner::normalize_actions(&self.actions);
        self.condition.normalize_and_validate()
    }
}
