mod keywords;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ModerationRuleLink {
    pub inclusive: bool,
    pub allowed: Vec<String>,
    pub blocked: Vec<String>,
    pub allow_top100: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "parameters")]
pub enum ModerationRuleType {
    Keywords(Vec<String>),
    Link(ModerationRuleLink),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ModerationRule {
    pub enabled: bool,
    #[serde(flatten)]
    pub rule_type: ModerationRuleType,
}

fn should_moderate_by_rule(message: &str, rule: &ModerationRule) -> Option<String> {
    if !rule.enabled {
        return None;
    }
    match &rule.rule_type {
        ModerationRuleType::Keywords(keywords) => keywords::should_moderate(message, keywords),
        ModerationRuleType::Link(_link_rule) => None, // TODO: implemented link moderation
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
