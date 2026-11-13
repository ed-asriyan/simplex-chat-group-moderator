//! `ContainsWords`: the message contains any of the owner's words, seen
//! through obfuscation.

mod filter;
mod upside_down;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::{Err, GroupMessage};
use crate::domain::moderator::rules::common::checks;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Maximum length (in characters) of a single keyword.
const MAX_KEYWORD_LENGTH: usize = 100;

/// The message contains any of `keywords`, seen through obfuscation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsWords {
    pub keywords: Vec<String>,
}

impl Condition for ContainsWords {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::normalize_list(&mut self.keywords);
        checks::check_list(&self.keywords, MAX_KEYWORD_LENGTH, "keywords", "Keyword")
    }

    fn describe(&self) -> String {
        "contains one of the listed words".into()
    }
}

#[async_trait]
impl ConditionOn<GroupMessage> for ContainsWords {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, GroupMessage>) -> Result<Option<String>, Err> {
        Ok(
            filter::should_moderate(ctx.subject.text.trim(), &self.keywords)
                .map(|keyword| format!("contains word: '{keyword}'")),
        )
    }
}
