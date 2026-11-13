//! What makes the bot check a rule, and what each trigger takes.
//!
//! A trigger is not a leaf: each one is driven by its own event (a method of
//! `ModerationEngine`), checks its own subject and is told to the owner in its
//! own words, so adding one is never a module and a registry line. What it
//! owns here is its module: the subject its rules are checked on, and the
//! lists of the conditions and actions it takes, from the leaves in
//! `moderation_condition/` and `moderation_action/`. A leaf belongs to no
//! trigger and to as many as list it; one a trigger lists must be checkable on
//! that trigger's subject, or the code does not compile.

use crate::domain::moderator::ports::Err;
use serde::{Deserialize, Serialize};

pub mod member_joined;
pub mod message_posted;

/// What makes the bot check a rule (US-D1, US-D6): the rule's `trigger` in the
/// rules JSON and the database.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// A member sends a message, or edits one (US-D1). Checked on the
    /// `GroupMessage`; [`message_posted`].
    #[default]
    MessagePosted,
    /// Someone joins the group as a member or an observer (US-D6). Checked on
    /// the `MemberJoin`; [`member_joined`].
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

/// The refusal for a condition or an action, by its serde tag `type_name`, in
/// a rule whose `trigger` does not take it (US-C2): `'Moderate the message'
/// does not work in a rule for when someone joins.`
pub(in crate::domain::moderator::rules) fn not_taken(type_name: &str, trigger: Trigger) -> Err {
    let _ = (type_name, trigger);
    todo!()
}
