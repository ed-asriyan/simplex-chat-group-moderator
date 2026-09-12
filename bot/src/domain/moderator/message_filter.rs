//! Message moderation: deciding what to do with a single group message.
//!
//! # What this module answers
//! Given one incoming message and a group's ordered list of rules, produce the set of
//! concrete actions to perform (moderate the message, set the author to observer, kick the
//! author), or nothing if no rule matches. Each [`ModerationRule`] pairs a *condition*
//! (does this message match?) with the *actions* to take if it matches.
//!
//! # Why it is not "first matching rule wins"
//! Multiple rules can match the same message, and their actions are not independent — some
//! actions subsume others. For example "moderate the message" is already implied by "kick the
//! author and delete all their messages". Naively applying every matched action, or only the
//! first, would either double-act or under-act. So actions are compared by a *covers*
//! (subset) relation and merged into a minimal, non-redundant plan. The same relation also
//! normalizes a single rule's own action list, which is a set rather than a sequence. All of
//! that comparison lives in the `action_planner` submodule; this module only orchestrates
//! rule evaluation.
//!
//! # The evaluation algorithm ([`should_moderate`])
//! We keep a growing list of planned [`ModerationAction`]s, starting empty, and walk the rules
//! **in their given order** (order is meaningful for tie-breaking and reasons):
//! 1. Before checking a rule's condition, ask `action_planner::plan_next_actions` what the
//!    plan *would become* if this rule applied.
//!    - `None` → every action of the rule is already covered by the current plan, so it can
//!      add nothing. We skip the condition check entirely (saves work, and for repo-backed
//!      conditions saves a query).
//!    - `Some(next)` → only now do we evaluate the condition. If it matches, the plan
//!      becomes `next` and the human-readable reason is recorded.
//! 2. After the loop, the planned actions are executed by the application layer in the order
//!    the planner emitted them (see `action_planner`'s ordering guarantees, e.g. moderate
//!    before kick).
//!
//! # The `AuthorHitsModerationRateLimit` special case (why there is a pre-pass)
//! `AuthorHitsModerationRateLimit` triggers based on how many of the author's *recent*
//! messages were moderated. The current message must count toward that total **iff it is
//! itself moderated by some other (independent) rule** — otherwise a single clean message
//! could never trip the limit, and a moderated one should. Whether the message is moderated
//! by another rule cannot depend on where that condition happens to sit (that would
//! make behaviour order-dependent, which we explicitly avoid).
//!
//! Since a condition is a tree, that condition can sit at any depth inside any rule, so
//! "every *other* rule" is no longer a well-defined set. The generalisation: **only when some
//! rule's tree contains one**, we make a pre-pass that evaluates every rule with all
//! `AuthorHitsModerationRateLimit` nodes pinned to "no match", and take the disjunction. When
//! the condition sits at the root of its own rule — the only shape expressible before trees —
//! this reduces exactly to the previous behaviour. `normalize_and_validate` forbids the
//! condition under a `Not`, which is what keeps the pinning from feeding back into itself.
//!
//! The pre-pass and the main loop share one memo (on `ConditionContext`), so each distinct
//! condition is still evaluated at most once per message.
//!
//! # Where things live
//! One module per entity, each owning the type and every submodule that only serves it:
//! - `moderation_condition` — [`ModerationCondition`]: its parameters, the checks applied when
//!   an owner saves them, and how one is evaluated against a message. The per-kind matching
//!   algorithms are its children (`keywords`, `links`, `regex_match`, `message_length`, ...).
//! - `moderation_action` — [`ModerationAction`]: one thing a matched rule does. A plain file,
//!   not a directory: unlike a condition, an action carries no per-kind logic of its own.
//! - `moderation_rule` — [`ModerationRule`]: the pairing of the two.
//!
//! Alongside them — deliberately *not* inside `moderation_action` — sits `action_planner`,
//! which merges the actions of several matched rules into one non-redundant plan and decides
//! the order they run in. It reasons about a *set* of actions coming from a *set* of rules, so
//! it belongs at the same level as this module's own [`should_moderate`], not one level down
//! inside the module that describes a single action. [`ModerationRule::normalize_and_validate`]
//! reaches into it for the same reason: one rule's action list is such a set too.
//!
//! This module is left with the orchestration across a set of rules ([`should_moderate`]) and
//! its result, [`ModerationMatch`].
//!
//! # Guidance for future changes
//! - Put action comparison / subsumption / ordering in the `action_planner` submodule, not
//!   here or in `moderation_action`. This module must not learn that "one rule is stronger than another"; it only asks
//!   the planner.
//! - Keep rule evaluation order-independent in outcome. If you need cross-rule context (like
//!   the pre-pass flag), compute it up front rather than relying on list position.
//! - Every condition should be evaluated at most once per message.

mod action_planner;
mod moderation_action;
mod moderation_condition;
mod moderation_rule;

#[cfg(test)]
mod tests;

pub use moderation_action::ModerationAction;
pub use moderation_condition::ModerationCondition;
pub use moderation_rule::ModerationRule;

use super::ports::{Err, GroupMessage, UserActivityRepository, UserModerationActivityRepository};
use moderation_condition::{ConditionContext, check_condition};

/// Result of evaluating message against moderation rules.
///
/// `actions` is the merged, non-redundant plan in execution order — it is what
/// the application performs and what the owner is told about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationMatch {
    pub actions: Vec<ModerationAction>,
    pub reasons: Vec<String>,
}

pub async fn should_moderate(
    group_message: &GroupMessage,
    rules: &[ModerationRule],
    activity_repo: &dyn UserActivityRepository,
    moderation_activity_repo: &dyn UserModerationActivityRepository,
) -> Result<Option<ModerationMatch>, Err> {
    let mut ctx = ConditionContext::new(group_message, activity_repo, moderation_activity_repo);

    // Only pay for the pre-pass when some rule actually asks how many of the
    // author's messages were moderated.
    if rules
        .iter()
        .any(|rule| rule.condition.contains_moderation_rate_limit())
    {
        ctx.moderation_rate_limit_pinned = true;
        let mut message_is_moderated = false;
        for rule in rules {
            if check_condition(&mut ctx, &rule.condition).await?.is_some() {
                message_is_moderated = true;
                break;
            }
        }
        ctx.moderation_rate_limit_pinned = false;
        ctx.message_is_moderated = message_is_moderated;
    }

    let mut current_actions: Vec<ModerationAction> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();

    for rule in rules {
        let candidate_actions =
            match action_planner::plan_next_actions(&current_actions, &rule.actions) {
                Some(actions) => actions,
                None => {
                    // Rule actions are a subset of already planned actions; skip condition check.
                    continue;
                }
            };

        if let Some(reason) = check_condition(&mut ctx, &rule.condition).await? {
            current_actions = candidate_actions;
            reasons.push(reason);
        }
    }

    if current_actions.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ModerationMatch {
            actions: current_actions,
            reasons,
        }))
    }
}
