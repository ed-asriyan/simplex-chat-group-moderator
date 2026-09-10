//! What makes a message match a rule.
//!
//! `ModerationCondition` is the single source of truth for detection types: the
//! PascalCase variant name is the serde tag used in the rules URL and
//! `rules-schema.json`, and its snake_case form names the condition's database
//! table. Everything a condition does lives here — its parameters, how they are
//! normalized and checked when an owner saves them, and how one is evaluated
//! against a message. The per-condition matching algorithms themselves live in
//! the sibling modules (`keywords`, `links`, `regex_match`, ...).

#[cfg(test)]
mod tests;

mod keywords;
mod links;
mod messages_blacklist;
mod moderation_rate_limit;
mod rate_limit;
mod regex_match;
mod screen_flooding;

use crate::domain::moderator::ports::{
    Err, GroupMessage, UserActivityRepository, UserModerationActivityRepository,
};
use regex::Regex;
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

/// Condition/filter criteria for moderation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationCondition {
    ContainsBannedWords {
        keywords: Vec<String>,
    },
    MatchesExactMessage {
        messages: Vec<String>,
        case_sensitive: bool,
    },
    MatchesRegex {
        patterns: Vec<String>,
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

// ---------------------------------------------------------------------------
// Saving: normalization and limits
// ---------------------------------------------------------------------------

/// Maximum number of entries in a single condition's keyword / message / domain list.
const MAX_LIST_ENTRIES: usize = 10_000;

/// Maximum length (in characters) of a single keyword or domain.
const MAX_KEYWORD_LENGTH: usize = 100;

/// Maximum length (in characters) of a single blacklisted exact message.
const MAX_MESSAGE_LENGTH: usize = 1000;

/// Maximum number of regex patterns in a single condition. Far lower than the other
/// lists because compiled patterns are memoized in a fixed-size process-wide cache
/// (see `regex_match`): this bounds how much of that shared cache one rule can claim,
/// keeping the cache useful for every other group.
const MAX_REGEX_PATTERNS: usize = 100;

/// Maximum length (in characters) of a single regex pattern.
const MAX_REGEX_PATTERN_LENGTH: usize = 200;

/// Drop blank entries and reduce the list to a sorted set. Blank entries are
/// dropped rather than rejected because the editor's list widget produces them
/// whenever a row is added and left untouched.
fn normalize_list(values: &mut Vec<String>) {
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
}

/// `noun` names the list in the "too many" message, `entry` a single entry in
/// the "too long" one, so both read naturally for keywords, messages and domains.
fn check_list(values: &[String], max_length: usize, noun: &str, entry: &str) -> Result<(), Err> {
    if values.len() > MAX_LIST_ENTRIES {
        return Err(format!(
            "Too many {noun}: {} provided, maximum is {MAX_LIST_ENTRIES}",
            values.len()
        )
        .into());
    }
    if let Some(value) = values.iter().find(|v| v.chars().count() > max_length) {
        return Err(format!(
            "{entry} too long: {} characters, maximum is {max_length}",
            value.chars().count()
        )
        .into());
    }
    Ok(())
}

impl ModerationCondition {
    /// Bring the condition's parameters into canonical form and check them
    /// against the limits.
    ///
    /// Normalization and validation are one step on purpose: validating without
    /// normalizing would count blank and duplicate entries towards the limits,
    /// and normalizing without validating would let oversized values through.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        match self {
            ModerationCondition::ContainsBannedWords { keywords } => {
                normalize_list(keywords);
                check_list(keywords, MAX_KEYWORD_LENGTH, "keywords", "Keyword")
            }
            ModerationCondition::MatchesExactMessage { messages, .. } => {
                normalize_list(messages);
                check_list(messages, MAX_MESSAGE_LENGTH, "messages", "Message")
            }
            ModerationCondition::MatchesRegex { patterns } => {
                normalize_list(patterns);
                if patterns.len() > MAX_REGEX_PATTERNS {
                    return Err(format!(
                        "Too many regex patterns: {} provided, maximum is {MAX_REGEX_PATTERNS}",
                        patterns.len()
                    )
                    .into());
                }
                if let Some(pattern) = patterns
                    .iter()
                    .find(|p| p.chars().count() > MAX_REGEX_PATTERN_LENGTH)
                {
                    return Err(format!(
                        "Regex pattern too long: {} characters, maximum is {MAX_REGEX_PATTERN_LENGTH}",
                        pattern.chars().count()
                    )
                    .into());
                }
                // Rejecting here is what lets the matcher treat a non-compiling
                // pattern as "never matches" instead of surfacing errors per message.
                if let Some(pattern) = patterns.iter().find(|p| Regex::new(p).is_err()) {
                    return Err(format!("Invalid regex pattern: '{pattern}'").into());
                }
                Ok(())
            }
            ModerationCondition::ContainsLinksToForbiddenWebsites { blocked } => {
                normalize_list(blocked);
                check_list(blocked, MAX_KEYWORD_LENGTH, "domains", "Domain")
            }
            ModerationCondition::ContainsLinksOutsideAllowedList { allowed }
            | ModerationCondition::ContainsLinksOutsideTop100 { allowed } => {
                normalize_list(allowed);
                check_list(allowed, MAX_KEYWORD_LENGTH, "domains", "Domain")
            }
            ModerationCondition::FloodsChatOrExceedsLimits { .. }
            | ModerationCondition::UserExceedsMessagesRateLimit { .. }
            | ModerationCondition::UserExceedsModerationRateLimit { .. } => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

fn should_moderate_by_condition(message: &str, condition: &ModerationCondition) -> Option<String> {
    match condition {
        ModerationCondition::ContainsBannedWords { keywords, .. } => {
            keywords::should_moderate(message.trim(), keywords)
                .map(|keyword| format!("blacklisted word: '{keyword}'"))
        }
        ModerationCondition::MatchesExactMessage {
            messages: blocked,
            case_sensitive,
        } => messages_blacklist::should_moderate(message.trim(), blocked, *case_sensitive),
        ModerationCondition::MatchesRegex { patterns } => {
            regex_match::should_moderate(message, patterns)
                .map(|pattern| format!("matches regex pattern: '{pattern}'"))
        }
        ModerationCondition::ContainsLinksToForbiddenWebsites { blocked } => {
            links::should_moderate_blacklist(message.trim(), blocked)
        }
        ModerationCondition::ContainsLinksOutsideAllowedList { allowed } => {
            links::should_moderate_whitelist(message.trim(), allowed)
        }
        ModerationCondition::ContainsLinksOutsideTop100 { allowed } => {
            links::should_moderate_whitelist_top100(message.trim(), allowed)
        }
        ModerationCondition::FloodsChatOrExceedsLimits {
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
        ModerationCondition::UserExceedsMessagesRateLimit { .. } => None,
        ModerationCondition::UserExceedsModerationRateLimit { .. } => None,
    }
}

pub(super) async fn check_condition(
    group_message: &GroupMessage,
    condition: &ModerationCondition,
    activity_repo: &dyn UserActivityRepository,
    moderation_activity_repo: &dyn UserModerationActivityRepository,
    current_message_is_moderated: bool,
) -> Result<Option<String>, Err> {
    match condition {
        ModerationCondition::UserExceedsMessagesRateLimit {
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
        ModerationCondition::UserExceedsModerationRateLimit {
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
        ModerationCondition::UserExceedsModerationRateLimit {
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
