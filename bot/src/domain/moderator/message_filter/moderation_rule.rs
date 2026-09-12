//! The pairing of a condition with the actions to take when it matches.

use super::{ModerationAction, ModerationCondition, action_planner};
use crate::domain::moderator::ports::Err;
use serde::{Deserialize, Serialize};

/// A moderation rule combining a filtering condition with the actions it triggers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationRule {
    pub actions: Vec<ModerationAction>,
    pub condition: ModerationCondition,
}

impl ModerationRule {
    /// Checks what an owner saved and rewrites it into the canonical form that
    /// is stored and evaluated.
    ///
    /// The action list is a set, so it is normalized through the planner the
    /// same way several matched rules are merged: duplicates and actions already
    /// covered by a stronger one disappear, and what is left is ordered the way
    /// it will be executed. A rule with no actions would detect something and
    /// then do nothing, so it is rejected rather than silently kept.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        if self.actions.is_empty() {
            return Err(
                "A rule has no actions. Add at least one action to it, or remove the rule.".into(),
            );
        }
        self.actions = action_planner::normalize_actions(&self.actions);
        self.condition.normalize_and_validate()
    }
}
