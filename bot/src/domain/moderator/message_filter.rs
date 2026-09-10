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
//! # Guidance for future changes
//! - Put action comparison / subsumption / ordering in the `action_planner` submodule, not
//!   here. This module must not learn that "one rule is stronger than another"; it only asks
//!   the planner.
//! - Keep rule evaluation order-independent in outcome. If you need cross-rule context (like
//!   the rate-limit flag), compute it up front rather than relying on list position.
//! - Every condition should be evaluated at most once per message.

mod action_planner;
mod keywords;
mod links;
mod messages_blacklist;
mod moderation_rate_limit;
mod rate_limit;
mod screen_flooding;

#[cfg(test)]
mod tests;

pub use action_planner::PlannedAction;

use super::ports::{Err, GroupMessage, UserActivityRepository, UserModerationActivityRepository};
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_chars_per_line() -> u32 {
    40
}

fn deserialize_u32_default_zero<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<u32>::deserialize(deserializer)?;
    Ok(opt.unwrap_or(0))
}

fn deserialize_chars_per_line<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<u32>::deserialize(deserializer)?;
    Ok(opt.unwrap_or(40))
}

/// Action to perform on the author's messages when kicking an author.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteAuthorMessages {
    None,
    #[default]
    TriggeredMessage,
    AllMessages,
}

/// Action to perform on the author's messages when setting an author as observer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteObserverMessages {
    None,
    #[default]
    TriggeredMessage,
}

/// Action to perform when a moderation rule triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationAction {
    #[default]
    ModerateMessage,
    KickAuthor {
        #[serde(default)]
        delete_messages: DeleteAuthorMessages,
    },
    SetAuthorObserver {
        #[serde(default)]
        delete_message: DeleteObserverMessages,
    },
}

/// Condition/filter criteria for moderation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleCondition {
    ContainsBannedWords {
        keywords: Vec<String>,
    },
    MatchesExactMessage {
        messages: Vec<String>,
        case_sensitive: bool,
    },
    ContainsLinksToForbiddenWebsites {
        blocked: Vec<String>,
    },
    ContainsLinksOutsideAllowedList {
        allowed: Vec<String>,
    },
    ContainsLinksOutsideTop100 {
        allowed: Vec<String>,
    },
    FloodsChatOrExceedsLimits {
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        max_characters: u32,
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        max_words: u32,
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        max_lines: u32,
        #[serde(
            default = "default_chars_per_line",
            deserialize_with = "deserialize_chars_per_line"
        )]
        chars_per_line: u32,
        #[serde(default = "default_true")]
        disallow_empty_messages: bool,
        #[serde(default)]
        disallow_invisible_chars: bool,
    },
    #[serde(alias = "RateLimit")]
    UserExceedsMessagesRateLimit {
        #[serde(
            default,
            deserialize_with = "deserialize_u32_default_zero",
            alias = "count",
            alias = "messages"
        )]
        message_count: u32,
        #[serde(
            default,
            deserialize_with = "deserialize_u32_default_zero",
            alias = "minutes",
            alias = "window_minutes"
        )]
        time_window_minutes: u32,
    },
    #[serde(alias = "UserExceededModerationRateLimit")]
    UserExceedsModerationRateLimit {
        #[serde(
            default,
            deserialize_with = "deserialize_u32_default_zero",
            alias = "count",
            alias = "moderated_count",
            alias = "messages"
        )]
        message_count: u32,
        #[serde(
            default,
            deserialize_with = "deserialize_u32_default_zero",
            alias = "minutes",
            alias = "window_minutes"
        )]
        time_window_minutes: u32,
    },
}

/// A moderation rule combining an action and a filtering condition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationRule {
    #[serde(default)]
    pub action: ModerationAction,
    pub condition: RuleCondition,
}

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

fn should_moderate_by_condition(message: &str, condition: &RuleCondition) -> Option<String> {
    match condition {
        RuleCondition::ContainsBannedWords { keywords, .. } => {
            keywords::should_moderate(message.trim(), keywords)
                .map(|keyword| format!("blacklisted word: '{keyword}'"))
        }
        RuleCondition::MatchesExactMessage {
            messages: blocked,
            case_sensitive,
        } => messages_blacklist::should_moderate(message.trim(), blocked, *case_sensitive),
        RuleCondition::ContainsLinksToForbiddenWebsites { blocked } => {
            links::should_moderate_blacklist(message.trim(), blocked)
        }
        RuleCondition::ContainsLinksOutsideAllowedList { allowed } => {
            links::should_moderate_whitelist(message.trim(), allowed)
        }
        RuleCondition::ContainsLinksOutsideTop100 { allowed } => {
            links::should_moderate_whitelist_top100(message.trim(), allowed)
        }
        RuleCondition::FloodsChatOrExceedsLimits {
            max_characters,
            max_words,
            max_lines,
            chars_per_line,
            disallow_invisible_chars,
            disallow_empty_messages,
        } => screen_flooding::should_moderate(
            message,
            *max_characters,
            *max_words,
            *max_lines,
            *chars_per_line,
            *disallow_invisible_chars,
            *disallow_empty_messages,
        ),
        RuleCondition::UserExceedsMessagesRateLimit { .. } => None,
        RuleCondition::UserExceedsModerationRateLimit { .. } => None,
    }
}

async fn check_condition(
    group_message: &GroupMessage,
    condition: &RuleCondition,
    activity_repo: &dyn UserActivityRepository,
    moderation_activity_repo: &dyn UserModerationActivityRepository,
    current_message_is_moderated: bool,
) -> Result<Option<String>, Err> {
    match condition {
        RuleCondition::UserExceedsMessagesRateLimit {
            message_count,
            time_window_minutes,
        } => {
            rate_limit::check_rate_limit(
                activity_repo,
                &group_message.group.id,
                &group_message.author_id,
                *message_count,
                *time_window_minutes,
                group_message.timestamp,
            )
            .await
        }
        RuleCondition::UserExceedsModerationRateLimit {
            message_count,
            time_window_minutes,
        } if current_message_is_moderated => {
            if *message_count == 0 || *time_window_minutes == 0 {
                Ok(None)
            } else {
                let since = group_message.timestamp
                    - chrono::Duration::minutes(*time_window_minutes as i64);
                let count = moderation_activity_repo
                    .count_moderated_messages_since(
                        &group_message.group.id,
                        &group_message.author_id,
                        since,
                        group_message.timestamp,
                    )
                    .await?
                    + 1;
                Ok(moderation_rate_limit::should_moderate(
                    count,
                    *message_count,
                    *time_window_minutes,
                ))
            }
        }
        RuleCondition::UserExceedsModerationRateLimit {
            message_count,
            time_window_minutes,
        } => {
            moderation_rate_limit::check_moderation_rate_limit(
                moderation_activity_repo,
                &group_message.group.id,
                &group_message.author_id,
                *message_count,
                *time_window_minutes,
                group_message.timestamp,
            )
            .await
        }
        other => Ok(should_moderate_by_condition(&group_message.text, other)),
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
            RuleCondition::UserExceedsModerationRateLimit { .. }
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

    let mut current_actions: Vec<action_planner::PlannedAction> = Vec::new();
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
