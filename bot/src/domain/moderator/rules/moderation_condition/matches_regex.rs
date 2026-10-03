//! `MatchesRegex`: the message matches any of the owner's regex patterns.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Maximum number of regex patterns in a single condition. Far lower than the other
/// lists because compiled patterns are memoized in a fixed-size process-wide cache
/// (see `filter`): this bounds how much of that shared cache one rule can claim,
/// keeping the cache useful for every other group.
const MAX_REGEX_PATTERNS: usize = 100;

/// Maximum length (in characters) of a single regex pattern.
const MAX_REGEX_PATTERN_LENGTH: usize = 200;

/// The message matches any of `patterns`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MatchesRegex {
    pub patterns: Vec<String>,
}

#[async_trait]
impl Condition for MatchesRegex {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        let patterns = &mut self.patterns;
        checks::normalize_list(patterns);
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

    fn describe(&self) -> String {
        "matches a regex pattern".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(
            filter::should_moderate(&ctx.group_message.text, &self.patterns)
                .map(|pattern| format!("matches regex pattern: '{pattern}'")),
        )
    }
}
