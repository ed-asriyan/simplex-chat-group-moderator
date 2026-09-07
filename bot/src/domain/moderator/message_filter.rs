mod keywords;
mod links;
mod messages_blacklist;
mod screen_flooding;

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_characters: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_words: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_lines: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chars_per_line: Option<u32>,
        #[serde(default)]
        disallow_invisible_chars: bool,
        #[serde(default = "default_true")]
        disallow_empty_messages: bool,
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
