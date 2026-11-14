//! Rules for the tests, built the way an owner writes them: from a
//! [`RuleDraft`], through the conversion that refuses what the trigger does
//! not take.

use super::{ModerationAction, ModerationCondition, ModerationRule, RuleDraft, Trigger};

/// A rule for when someone sends a message, on, with no name.
pub fn message_rule(
    actions: Vec<ModerationAction>,
    condition: ModerationCondition,
) -> ModerationRule {
    draft_rule(RuleDraft {
        name: None,
        trigger: Trigger::MessagePosted,
        enabled: true,
        notify: true,
        condition: Some(condition),
        actions,
    })
}

/// A rule for when someone joins, on, with no name; no `condition` matches
/// everyone who joins.
pub fn join_rule(
    actions: Vec<ModerationAction>,
    condition: Option<ModerationCondition>,
) -> ModerationRule {
    draft_rule(RuleDraft {
        name: None,
        trigger: Trigger::MemberJoined,
        enabled: true,
        notify: true,
        condition,
        actions,
    })
}

/// The rule a draft is, which the test says is a valid one.
pub fn draft_rule(draft: RuleDraft) -> ModerationRule {
    ModerationRule::try_from(draft).expect("a valid rule")
}

/// `rule` with a name and, if `enabled` is false, its actions switched off.
pub fn named(mut rule: ModerationRule, name: &str, enabled: bool) -> ModerationRule {
    rule.name = Some(name.to_string());
    rule.enabled = enabled;
    rule
}

/// `rule` with its two switches set: its actions (`enabled`) and its
/// notifications (`notify`).
pub fn switched(mut rule: ModerationRule, enabled: bool, notify: bool) -> ModerationRule {
    rule.enabled = enabled;
    rule.notify = notify;
    rule
}
