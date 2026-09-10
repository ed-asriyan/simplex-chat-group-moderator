//! The pairing of a condition with the action to take when it matches.

use super::{ModerationAction, ModerationCondition};
use serde::{Deserialize, Serialize};

/// A moderation rule combining an action and a filtering condition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationRule {
    #[serde(default)]
    pub action: ModerationAction,
    pub condition: ModerationCondition,
}
