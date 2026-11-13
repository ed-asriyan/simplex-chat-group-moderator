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
//! 2. After the loop, the application hands the planned actions to [`execute_actions`], which
//!    carries them out in the order the planner emitted them (see `action_planner`'s ordering
//!    guarantees, e.g. moderate before kick).
//!
//! # Conditions that depend on the other rules (why there is a pre-pass)
//! Some conditions answer a question about the *other* rules. The one there is today,
//! `AuthorHitsModerationRateLimit`, triggers based on how many of the author's *recent*
//! messages were moderated, and the current message must count toward that total **iff it is
//! itself moderated by some other (independent) rule** — otherwise a single clean message
//! could never trip the limit, and a moderated one should. Whether the message is moderated
//! by another rule cannot depend on where that condition happens to sit (that would
//! make behaviour order-dependent, which we explicitly avoid).
//!
//! Since a condition is a tree, such a condition can sit at any depth inside any rule, so
//! "every *other* rule" is no longer a well-defined set. The generalisation: **only when some
//! rule's tree contains one** (`Condition::depends_on_other_rules`), we make a pre-pass that
//! evaluates every rule with all such nodes pinned to "no match", and take the disjunction.
//! When the condition sits at the root of its own rule — the only shape expressible before
//! trees — this reduces exactly to the previous behaviour. `normalize_and_validate` forbids
//! such a condition under a `Not`, which is what keeps the pinning from feeding back into
//! itself. This module never names the condition: it only asks the tree.
//!
//! The pre-pass and the main loop share one memo (on `ConditionContext`), so each distinct
//! condition is still evaluated at most once per message.
//!
//! The pre-pass's reason is also kept for the owner. The main loop skips a rule whose actions
//! are already covered, and the rule that moderated this message is often covered by the very
//! kick the rate limit triggers — so its reason is appended if the loop did not record it.
//!
//! # Where things live
//! One module per entity, each owning the type and every submodule that only serves it:
//! - `moderation_condition` — [`ModerationCondition`]: the registry of every condition and
//!   the tree they form. Each leaf condition is a child module of its own (`contains_words`,
//!   `exceeds_max_lines`, ...) holding its parameters, the checks applied when an owner saves
//!   them, how it is evaluated against a message, and its tests.
//! - `moderation_action` — [`ModerationAction`]: the registry of every action. Each action is a
//!   child module of its own (`moderate_message`, `kick_author`, ...) holding its parameters,
//!   what it achieves (`Effect`), where it goes in the execution order, how it is carried out
//!   ([`execute_actions`]), and its tests.
//! - `moderation_rule` — [`ModerationRule`]: the pairing of the two.
//! - `bookkeeping` — what the bot records about every message so the conditions can answer
//!   later, from what each condition says it needs.
//! - `common` — what two conditions or actions share. It knows none of them, and nothing of
//!   the engine around them.
//!
//! Alongside them — deliberately *not* inside `moderation_action` — sits `action_planner`,
//! which merges the actions of several matched rules into one non-redundant plan and decides
//! the order they run in, from what each action says it achieves and where it goes. It
//! reasons about a *set* of actions coming from a *set* of rules, so it belongs at the same
//! level as this module's own [`should_moderate`], not one level down inside the module that
//! describes a single action. [`ModerationRule::normalize_and_validate`] reaches into it for
//! the same reason: one rule's action list is such a set too.
//!
//! This module is left with the orchestration across a set of rules ([`should_moderate`]) and
//! its result, [`ModerationMatch`].
//!
//! # Guidance for future changes
//! - What one action achieves and where it goes in the order belong to that action
//!   (`Action::effect`, `Action::execution_rank`); how a set of them combines belongs to
//!   `action_planner`. This module must not learn that "one rule is stronger than another"; it
//!   only asks the planner.
//! - Keep rule evaluation order-independent in outcome. If you need cross-rule context (like
//!   the pre-pass flag), compute it up front rather than relying on list position.
//! - Every condition should be evaluated at most once per message.
//! - A condition or an action is named only by its own module and its registry line. The
//!   engine asks it through its trait (`Condition`, `Action`); one leaf never imports another,
//!   and what two of them share goes in `common`. `architecture_tests` checks this on the
//!   source.

mod action_planner;
mod bookkeeping;
mod common;
mod moderation_action;
mod moderation_condition;
mod moderation_rule;
mod trigger;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod architecture_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
pub mod test_support;

pub use bookkeeping::{record_activity, record_moderated, record_outcome};
pub use moderation_action::{
    ActionOutcome, ActionPorts, ModerationAction, actions, execute_join_actions,
    execute_message_actions,
};
pub use moderation_condition::{ConditionPorts, KeyUse, ModerationCondition, conditions};
pub use moderation_rule::{MAX_RULE_NAME_CHARS, ModerationRule, RuleDraft, TriggeredRule};
pub use trigger::Trigger;
pub use trigger::member_joined::{JoinAction, JoinCondition};
pub use trigger::message_posted::{MessageAction, MessageCondition};

use super::ports::{Err, GroupMessage, MemberJoin, RuleReason};

/// What the rules of one trigger call for about one message or one join.
///
/// `actions` — the trigger's own action type `A` — is the merged,
/// non-redundant plan in execution order: it is what the application performs
/// and what the owner is told about. `reasons` has one entry per matched
/// rule, in the order of the rules (US-D2, US-D6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationMatch<A> {
    pub actions: Vec<A>,
    pub reasons: Vec<RuleReason>,
}

impl<A: Copy> ModerationMatch<A>
where
    ModerationAction: From<A>,
{
    /// The planned actions as an owner writes them, which is how the notification
    /// names them.
    pub fn actions_as_written(&self) -> Vec<ModerationAction> {
        self.actions
            .iter()
            .copied()
            .map(ModerationAction::from)
            .collect()
    }
}

/// Checks one message against the group's rules for `Trigger::MessagePosted`
/// that are on, in order, and merges what the matching ones call for (US-D1).
/// Rules for other triggers and rules that are off are skipped and cover
/// nothing; `now` is the message's own time.
pub async fn should_moderate_message(
    group_message: &GroupMessage,
    rules: &[ModerationRule],
    ports: &ConditionPorts<'_>,
) -> Result<Option<ModerationMatch<MessageAction>>, Err> {
    let _ = (group_message, rules, ports);
    todo!()
}

/// Checks someone who joined against the group's rules for
/// `Trigger::MemberJoined` that are on, in order, and merges what the matching
/// ones call for; a rule with no condition matches everyone (US-D6).
pub async fn should_moderate_join(
    join: &MemberJoin,
    rules: &[ModerationRule],
    ports: &ConditionPorts<'_>,
) -> Result<Option<ModerationMatch<JoinAction>>, Err> {
    let _ = (join, rules, ports);
    todo!()
}
