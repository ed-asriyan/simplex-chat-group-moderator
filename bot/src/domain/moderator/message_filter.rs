mod keywords;
mod links;
mod messages_blacklist;
mod screen_flooding;

#[cfg(test)]
mod tests;

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
    WordsBlacklist {
        keywords: Vec<String>,
    },
    MessagesBlacklist {
        messages: Vec<String>,
        case_sensitive: bool,
    },
    LinksBlacklist {
        blocked: Vec<String>,
    },
    LinksWhitelist {
        allowed: Vec<String>,
    },
    LinksWhitelistTop100 {
        allowed: Vec<String>,
    },
    ScreenFlooding {
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
        RuleCondition::WordsBlacklist { keywords, .. } => {
            keywords::should_moderate(message.trim(), keywords)
                .map(|keyword| format!("blacklisted word: '{keyword}'"))
        }
        RuleCondition::MessagesBlacklist {
            messages: blocked,
            case_sensitive,
        } => messages_blacklist::should_moderate(message.trim(), blocked, *case_sensitive),
        RuleCondition::LinksBlacklist { blocked } => {
            links::should_moderate_blacklist(message.trim(), blocked)
        }
        RuleCondition::LinksWhitelist { allowed } => {
            links::should_moderate_whitelist(message.trim(), allowed)
        }
        RuleCondition::LinksWhitelistTop100 { allowed } => {
            links::should_moderate_whitelist_top100(message.trim(), allowed)
        }
        RuleCondition::ScreenFlooding {
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
    }
}

pub fn should_moderate(message: &str, rules: &[ModerationRule]) -> Option<ModerationMatch> {
    for rule in rules {
        if let Some(reason) = should_moderate_by_condition(message, &rule.condition) {
            return Some(ModerationMatch {
                action: rule.action,
                reason,
            });
        }
    }
    None
}
