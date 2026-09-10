mod keywords;
mod links;
mod messages_blacklist;
mod moderation_rate_limit;
mod rate_limit;
mod screen_flooding;

#[cfg(test)]
mod tests;

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
    pub action: ModerationAction,
    pub reason: String,
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

pub async fn should_moderate(
    group_message: &GroupMessage,
    rules: &[ModerationRule],
    activity_repo: &dyn UserActivityRepository,
    moderation_activity_repo: &dyn UserModerationActivityRepository,
) -> Result<Option<ModerationMatch>, Err> {
    for rule in rules {
        let reason = match &rule.condition {
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
                .await?
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
                .await?
            }
            other => should_moderate_by_condition(&group_message.text, other),
        };

        if let Some(reason) = reason {
            return Ok(Some(ModerationMatch {
                action: rule.action,
                reason,
            }));
        }
    }
    Ok(None)
}
