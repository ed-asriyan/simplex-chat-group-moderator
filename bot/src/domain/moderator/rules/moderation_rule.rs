//! A rule: what makes the bot check it, what it checks, and what it does on a
//! match — named by the owner, with its two switches, *Actions* and
//! *Notifications* (US-D1–US-D4, US-D6, US-G13, US-G19).
//!
//! A rule exists in two forms. [`RuleDraft`] is the rule as an owner writes it
//! — the rules JSON, the editor link —, any condition and any action next to
//! any trigger. [`ModerationRule`] is the rule the bot keeps and checks: its
//! condition and its actions are its trigger's own types, so a rule that holds
//! something its trigger does not take cannot exist. A draft becomes a rule by
//! `TryFrom`, which is where such a rule is refused (US-C2); the JSON of a
//! `ModerationRule` is its draft's, so the refusal is what the owner reads
//! when the configuration they send does not parse.

#[cfg(test)]
mod tests;

use super::trigger::Trigger;
use super::trigger::member_joined::{JoinAction, JoinCondition};
use super::trigger::message_posted::{MessageAction, MessageCondition};
use super::{KeyUse, ModerationAction, ModerationCondition, action_planner};
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
    /// *Actions*: whether the bot carries out the rule's actions when it
    /// matches. With them off, the rule is still checked while `notify` is on,
    /// its actions cover nothing for the rules with them on, and the owner is
    /// told what it would have done (US-D1, US-D4, US-G13).
    pub enabled: bool,
    /// *Notifications*: whether the owner is told when the rule matches
    /// (US-D2, US-D3, US-G19). A rule with both switches off is kept with its
    /// settings and sent back in the editor link, but never checked.
    pub notify: bool,
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
        match &self.body {
            TriggeredRule::MessagePosted { condition, .. } => {
                ModerationCondition::from(condition.clone()).key_uses()
            }
            TriggeredRule::MemberJoined { condition, .. } => condition
                .clone()
                .map(|condition| ModerationCondition::from(condition).key_uses())
                .unwrap_or_default(),
        }
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
        if let Some(name) = &self.name {
            if name.chars().count() > MAX_RULE_NAME_CHARS {
                return Err(format!(
                    "A rule's name is longer than {MAX_RULE_NAME_CHARS} characters."
                )
                .into());
            }
            if name.trim().is_empty() {
                self.name = None;
            }
        }
        match &mut self.body {
            TriggeredRule::MessagePosted { condition, actions } => {
                *actions = normalized_actions(actions)?;
                *condition = normalized_condition(condition.clone())?;
            }
            TriggeredRule::MemberJoined { condition, actions } => {
                *actions = normalized_actions(actions)?;
                if let Some(inner) = condition.take() {
                    *condition = Some(normalized_condition(inner)?);
                }
            }
        }
        Ok(())
    }
}

/// A rule as an owner writes it, in the rules JSON: `{"name": ..., "trigger":
/// ..., "enabled": ..., "notify": ..., "condition": {...}, "actions": [...]}`.
/// A missing `trigger` is a message rule, a missing `enabled` or `notify` a
/// switch that is on, and a missing `name` or `condition` is none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDraft {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Written only when it is not the default, so a message rule reads as it
    /// always has.
    #[serde(default, skip_serializing_if = "is_default_trigger")]
    pub trigger: Trigger,
    /// Written only when the rule's actions are off.
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_enabled")]
    pub enabled: bool,
    /// Written only when the rule's notifications are off.
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_enabled")]
    pub notify: bool,
    pub actions: Vec<ModerationAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ModerationCondition>,
}

fn is_default_trigger(trigger: &Trigger) -> bool {
    *trigger == Trigger::default()
}

fn is_enabled(enabled: &bool) -> bool {
    *enabled
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
        let RuleDraft {
            name,
            trigger,
            enabled,
            notify,
            condition,
            actions,
        } = draft;
        let body = match trigger {
            Trigger::MessagePosted => TriggeredRule::MessagePosted {
                condition: condition
                    .ok_or_else(|| -> Err {
                        "A rule for when someone sends a message has no condition. Add one, or remove the rule."
                            .into()
                    })?
                    .try_into()?,
                actions: narrowed(actions)?,
            },
            Trigger::MemberJoined => TriggeredRule::MemberJoined {
                condition: condition.map(TryInto::try_into).transpose()?,
                actions: narrowed(actions)?,
            },
        };
        Ok(Self {
            name,
            enabled,
            notify,
            body,
        })
    }
}

impl From<ModerationRule> for RuleDraft {
    fn from(rule: ModerationRule) -> Self {
        let (trigger, condition, actions) = match rule.body {
            TriggeredRule::MessagePosted { condition, actions } => (
                Trigger::MessagePosted,
                Some(condition.into()),
                actions.into_iter().map(Into::into).collect(),
            ),
            TriggeredRule::MemberJoined { condition, actions } => (
                Trigger::MemberJoined,
                condition.map(Into::into),
                actions.into_iter().map(Into::into).collect(),
            ),
        };
        Self {
            name: rule.name,
            trigger,
            enabled: rule.enabled,
            notify: rule.notify,
            condition,
            actions,
        }
    }
}

/// A trigger's actions from the actions as written; the first one the trigger
/// does not take refuses the rule.
fn narrowed<A: TryFrom<ModerationAction, Error = Err>>(
    actions: Vec<ModerationAction>,
) -> Result<Vec<A>, Err> {
    actions.into_iter().map(A::try_from).collect()
}

/// A trigger's actions checked and put into the canonical form: each action's
/// own settings first, then the planner's merge and order (US-F4).
fn normalized_actions<A>(actions: &[A]) -> Result<Vec<A>, Err>
where
    A: Copy + TryFrom<ModerationAction, Error = Err>,
    ModerationAction: From<A>,
{
    if actions.is_empty() {
        return Err(
            "A rule has no actions. Add at least one action to it, or remove the rule.".into(),
        );
    }
    let mut written: Vec<ModerationAction> = actions.iter().copied().map(Into::into).collect();
    for action in &mut written {
        action.normalize_and_validate()?;
    }
    action_planner::normalize_actions(&written)
        .into_iter()
        .map(A::try_from)
        .collect()
}

/// A trigger's condition tree checked and put into the canonical form.
fn normalized_condition<C>(condition: C) -> Result<C, Err>
where
    C: TryFrom<ModerationCondition, Error = Err>,
    ModerationCondition: From<C>,
{
    let mut written = ModerationCondition::from(condition);
    written.normalize_and_validate()?;
    C::try_from(written)
}
