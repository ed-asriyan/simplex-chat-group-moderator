//! Message moderation: deciding what to do with a single group message.
//!
//! # What this module answers
//! Given one incoming message and a group's ordered list of rules, produce the set of
//! concrete actions to perform (delete the message, set the author to observer, kick the
//! author), or nothing if no rule matches. Each [`ModerationRule`] pairs a *condition*
//! (does this message match?) with an *action* (what to do if it matches).
//!
//! # Why it is not "first matching rule wins"
//! Multiple rules can match the same message, and their actions are not independent — some
//! actions subsume others. For example "delete the message" is already implied by "kick the
//! author and delete all their messages". Naively applying every matched action, or only the
//! first, would either double-act or under-act. So actions are compared by a *covers*
//! (subset) relation and merged into a minimal, non-redundant plan. All of that comparison
//! lives in the `action_planner` submodule; this module only orchestrates rule evaluation.
//!
//! # The evaluation algorithm ([`should_moderate`])
//! We keep a growing list of planned [`PlannedAction`]s, starting empty, and walk the rules
//! **in their given order** (order is meaningful for tie-breaking and reasons):
//! 1. Before checking a rule's condition, ask `action_planner::plan_next_actions` what the
//!    plan *would become* if this rule applied.
//!    - `None` → the rule's action is already covered by the current plan, so it can add
//!      nothing. We skip the condition check entirely (saves work, and for repo-backed
//!      conditions saves a query).
//!    - `Some(next)` → only now do we evaluate the condition. If it matches, the plan
//!      becomes `next` and the human-readable reason is recorded.
//! 2. After the loop, the planned actions are executed by the application layer in the order
//!    the planner emitted them (see `action_planner`'s ordering guarantees, e.g. delete
//!    before kick).
//!
//! # The moderation-rate-limit special case (why there is a pre-pass)
//! `UserExceedsModerationRateLimit` triggers based on how many of the author's *recent*
//! messages were moderated. The current message must count toward that total **iff it is
//! itself moderated by some other (independent) rule** — otherwise a single clean message
//! could never trip the limit, and a moderated one should. Whether the message is moderated
//! by another rule cannot depend on where the rate-limit rule happens to sit in the list
//! (that would make behaviour order-dependent, which we explicitly avoid).
//!
//! Therefore, **only when a `UserExceedsModerationRateLimit` rule exists**, we do a single
//! pre-pass that evaluates every *other* condition once, caches each result, and derives a
//! single order-independent flag `message_is_moderated`. The main loop then reuses those
//! cached results (so every condition is evaluated exactly once) and passes the flag into the
//! rate-limit condition, which adds `+1` for the current message when the flag is set. When no
//! such rule exists, the pre-pass is skipped and conditions are evaluated lazily in the main
//! loop.
//!
//! # Where things live
//! One module per entity, each owning the type and every submodule that only serves it:
//! - `moderation_condition` — [`ModerationCondition`]: its parameters, the checks applied when
//!   an owner saves them, and how one is evaluated against a message. The per-kind matching
//!   algorithms are its children (`keywords`, `links`, `regex_match`, `screen_flooding`, ...).
//! - `moderation_action` — [`ModerationAction`]: what a matched rule does. A plain file, not a
//!   directory: unlike a condition, an action carries no per-kind logic of its own.
//! - `moderation_rule` — [`ModerationRule`]: the pairing of the two.
//!
//! Alongside them — deliberately *not* inside `moderation_action` — sits `action_planner`,
//! which owns [`PlannedAction`] and merges the actions of several matched rules into one
//! non-redundant plan. It reasons about a *set* of actions coming from a *set* of rules, so it
//! belongs at the same level as this module's own [`should_moderate`], not one level down
//! inside the module that describes a single action.
//!
//! This module is left with the orchestration across a set of rules ([`should_moderate`]) and
//! its result, [`ModerationMatch`].
//!
//! # Guidance for future changes
//! - Put action comparison / subsumption / ordering in the `action_planner` submodule, not
//!   here. This module must not learn that "one rule is stronger than another"; it only asks
//!   the planner.
//! - Keep rule evaluation order-independent in outcome. If you need cross-rule context (like
//!   the rate-limit flag), compute it up front rather than relying on list position.
//! - Every condition should be evaluated at most once per message.

mod action_planner;
mod moderation_action;
mod moderation_condition;
mod moderation_rule;

#[cfg(test)]
mod tests;

pub use action_planner::PlannedAction;
pub use moderation_action::{DeleteAuthorMessages, DeleteObserverMessages, ModerationAction};
pub use moderation_condition::ModerationCondition;
pub use moderation_rule::ModerationRule;

use super::ports::{Err, GroupMessage, UserActivityRepository, UserModerationActivityRepository};
use moderation_condition::check_condition;

/// Result of evaluating message against moderation rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationMatch {
    pub actions: Vec<PlannedAction>,
    pub reason: String,
}

impl ModerationMatch {
    /// Consolidated `ModerationAction` representation (e.g. for owner notifications).
    pub fn action(&self) -> ModerationAction {
        action_planner::planned_actions_to_moderation_action(&self.actions)
    }
}

pub async fn should_moderate(
    group_message: &GroupMessage,
    rules: &[ModerationRule],
    activity_repo: &dyn UserActivityRepository,
    moderation_activity_repo: &dyn UserModerationActivityRepository,
) -> Result<Option<ModerationMatch>, Err> {
    let is_moderation_rate = |rule: &ModerationRule| {
        matches!(
            rule.condition,
            ModerationCondition::UserExceedsModerationRateLimit { .. }
        )
    };

    // A moderation-rate condition needs to know, independently of rule order, whether this
    // message is moderated by another rule. Only when such a condition exists do we evaluate
    // every other condition up front and cache the result so each is computed exactly once.
    let has_moderation_rate_rule = rules.iter().any(is_moderation_rate);
    let mut cached_reasons: Vec<Option<String>> = Vec::new();
    let mut message_is_moderated = false;
    if has_moderation_rate_rule {
        for rule in rules {
            let reason = if is_moderation_rate(rule) {
                None
            } else {
                check_condition(
                    group_message,
                    &rule.condition,
                    activity_repo,
                    moderation_activity_repo,
                    false,
                )
                .await?
            };
            message_is_moderated |= reason.is_some();
            cached_reasons.push(reason);
        }
    }

    let mut current_actions: Vec<PlannedAction> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();

    for (index, rule) in rules.iter().enumerate() {
        let candidate_actions =
            match action_planner::plan_next_actions(&current_actions, &rule.action) {
                Some(actions) => actions,
                None => {
                    // Rule action is a subset of already planned actions; skip condition check.
                    continue;
                }
            };

        let reason = if has_moderation_rate_rule && !is_moderation_rate(rule) {
            cached_reasons[index].clone()
        } else {
            check_condition(
                group_message,
                &rule.condition,
                activity_repo,
                moderation_activity_repo,
                message_is_moderated,
            )
            .await?
        };

        if let Some(reason) = reason {
            current_actions = candidate_actions;
            reasons.push(reason);
        }
    }

    if current_actions.is_empty() {
        Ok(None)
    } else {
        let reason = reasons.join(", ");
        Ok(Some(ModerationMatch {
            actions: current_actions,
            reason,
        }))
    }
}
