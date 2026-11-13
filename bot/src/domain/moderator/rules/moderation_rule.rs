//! A rule: what makes the bot check it, what it checks, and what it does on a
//! match — named by the owner and turned on or off by them (US-D1, US-D6,
//! US-G13).
//!
//! A rule exists in two forms. [`RuleDraft`] is the rule as an owner writes it
//! — the rules JSON, the editor link —, any condition and any action next to
//! any trigger. [`ModerationRule`] is the rule the bot keeps and checks: its
//! condition and its actions are its trigger's own types, so a rule that holds
//! something its trigger does not take cannot exist. A draft becomes a rule by
//! `TryFrom`, which is where such a rule is refused (US-C2); the JSON of a
//! `ModerationRule` is its draft's, so the refusal is what the owner reads
//! when the configuration they send does not parse.

use super::trigger::Trigger;
use super::trigger::member_joined::{JoinAction, JoinCondition};
use super::trigger::message_posted::{MessageAction, MessageCondition};
use super::{KeyUse, ModerationAction, ModerationCondition};
use crate::domain::moderator::ports::Err;
use serde::{Deserialize, Serialize};

/// The longest name a rule may have, in characters (US-C2).
pub const MAX_RULE_NAME_CHARS: usize = 60;

/// A moderation rule as the bot keeps and checks it: on its trigger, when its
/// condition matches, the bot carries out its actions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RuleDraft", into = "RuleDraft")]
pub struct ModerationRule {
    /// What the owner calls the rule: at most [`MAX_RULE_NAME_CHARS`]
    /// characters, never blank. The notifications name a matched rule by it
    /// (US-D2, US-D6); a rule without one is reported by what matched alone.
    pub name: Option<String>,
    /// A rule that is off is kept with its settings and sent back in the
    /// editor link, but never checked, and its actions cover nothing (US-D1,
    /// US-D6, US-G13).
    pub enabled: bool,
    pub body: TriggeredRule,
}

/// A rule's trigger, with the condition and the actions of that trigger's own
/// types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggeredRule {
    MessagePosted {
        condition: MessageCondition,
        actions: Vec<MessageAction>,
    },
    MemberJoined {
        /// `None` matches everyone who joins (US-D6).
        condition: Option<JoinCondition>,
        actions: Vec<JoinAction>,
    },
}

impl ModerationRule {
    pub fn trigger(&self) -> Trigger {
        match self.body {
            TriggeredRule::MessagePosted { .. } => Trigger::MessagePosted,
            TriggeredRule::MemberJoined { .. } => Trigger::MemberJoined,
        }
    }

    /// The API keys the rule's conditions carry, and what each is for, so they
    /// can be checked when an owner saves the rule (US-C4).
    pub fn key_uses(&self) -> Vec<KeyUse> {
        todo!()
    }

    /// Checks what an owner saved and rewrites it into the canonical form that
    /// is stored and evaluated (US-C2).
    ///
    /// Refuses a rule with no actions and a name longer than
    /// [`MAX_RULE_NAME_CHARS`]; a blank name becomes none. Each action checks
    /// its own parameters first, then the list is normalized through the
    /// planner the same way several matched rules are merged: duplicates and
    /// actions already covered by a stronger one disappear, and what is left is
    /// ordered the way it will be executed (US-F4). The order matters — the
    /// planner compares action parameters, so they have to be settled by then.
    /// The condition tree is normalized and kept within its limits. A rule
    /// that is off is checked the same: the owner can turn it on at any time
    /// without saving it again.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        todo!()
    }
}

/// A rule as an owner writes it, in the rules JSON: `{"name": ..., "trigger":
/// ..., "enabled": ..., "condition": {...}, "actions": [...]}`. A missing
/// `trigger` is a message rule, a missing `enabled` a rule that is on, and a
/// missing `name` or `condition` is none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDraft {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ModerationCondition>,
    pub actions: Vec<ModerationAction>,
}

fn enabled_by_default() -> bool {
    true
}

impl TryFrom<RuleDraft> for ModerationRule {
    type Error = Err;

    /// Refuses a message rule with no condition (`A rule for when someone
    /// sends a message has no condition. Add one, or remove the rule.`), and a
    /// condition or an action the rule's trigger does not take (`'Moderate the
    /// message' does not work in a rule for when someone joins.`) — US-C2. It
    /// checks nothing else: the rest is
    /// [`ModerationRule::normalize_and_validate`]'s.
    fn try_from(draft: RuleDraft) -> Result<Self, Self::Error> {
        let _ = draft;
        todo!()
    }
}

impl From<ModerationRule> for RuleDraft {
    fn from(rule: ModerationRule) -> Self {
        let _ = rule;
        todo!()
    }
}
