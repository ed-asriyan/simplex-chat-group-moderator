//! `ContainsWords`: the message contains any of the owner's words, seen
//! through obfuscation.

use super::keywords;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
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

#[async_trait]
impl Condition for ContainsWords {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::normalize_list(&mut self.keywords);
        checks::check_list(&self.keywords, MAX_KEYWORD_LENGTH, "keywords", "Keyword")
    }

    fn describe(&self) -> String {
        "contains one of the listed words".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(
            keywords::should_moderate(ctx.group_message.text.trim(), &self.keywords)
                .map(|keyword| format!("contains word: '{keyword}'")),
        )
    }
}
