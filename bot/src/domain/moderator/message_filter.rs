mod keywords;
mod links;
mod messages_blacklist;

use serde::{Deserialize, Serialize};

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
}

fn should_moderate_by_rule(message: &str, rule: &ModerationRule) -> Option<String> {
    match rule {
        ModerationRule::WordsBlacklist { keywords, .. } => {
            keywords::should_moderate(message, keywords)
        }
        ModerationRule::MessagesBlacklist {
            messages: blocked,
            case_sensitive,
        } => messages_blacklist::should_moderate(message, blocked, *case_sensitive),
        ModerationRule::LinksBlacklist { blocked } => {
            links::should_moderate_blacklist(message, blocked)
        }
        ModerationRule::LinksWhitelist { allowed } => {
            links::should_moderate_whitelist(message, allowed)
        }
        ModerationRule::LinksWhitelistTop100 { allowed } => {
            links::should_moderate_whitelist_top100(message, allowed)
        }
    }
}

pub fn should_moderate(message: &str, rules: &[ModerationRule]) -> Option<String> {
    let message = message.trim();
    for rule in rules {
        if let Some(reason) = should_moderate_by_rule(message, rule) {
            return Some(reason);
        }
    }
    None
}
