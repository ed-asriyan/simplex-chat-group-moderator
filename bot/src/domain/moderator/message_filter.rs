mod keywords;
mod links;
mod messages_blacklist;
mod screen_flooding;

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

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationRule {
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

fn should_moderate_by_rule(message: &str, rule: &ModerationRule) -> Option<String> {
    match rule {
        ModerationRule::WordsBlacklist { keywords, .. } => {
            keywords::should_moderate(message.trim(), keywords)
        }
        ModerationRule::MessagesBlacklist {
            messages: blocked,
            case_sensitive,
        } => messages_blacklist::should_moderate(message.trim(), blocked, *case_sensitive),
        ModerationRule::LinksBlacklist { blocked } => {
            links::should_moderate_blacklist(message.trim(), blocked)
        }
        ModerationRule::LinksWhitelist { allowed } => {
            links::should_moderate_whitelist(message.trim(), allowed)
        }
        ModerationRule::LinksWhitelistTop100 { allowed } => {
            links::should_moderate_whitelist_top100(message.trim(), allowed)
        }
        ModerationRule::ScreenFlooding {
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

pub fn should_moderate(message: &str, rules: &[ModerationRule]) -> Option<String> {
    for rule in rules {
        if let Some(reason) = should_moderate_by_rule(message, rule) {
            return Some(reason);
        }
    }
    None
}
