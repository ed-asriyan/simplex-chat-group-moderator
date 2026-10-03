//! `FlaggedByOmniModeration`: OpenAI's moderation model, asked with the
//! owner's own key, trips one of the owner's per-category triggers.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::{
    ApiRetry, CategoryTrigger, Err, OpenAiCategory, OpenAiCategoryTriggers,
};
use crate::domain::moderator::rules::common::api_key::normalize as normalize_api_key;
use crate::domain::moderator::rules::common::api_retry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const TITLE: &str = "Flagged by OpenAI Omni";

/// OpenAI's moderation model, asked with the owner's own `api_key`, trips
/// at least one of the owner's per-category `triggers`. Sends the message
/// text to OpenAI; a message with no text is never sent and never matches.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlaggedByOmniModeration {
    pub api_key: String,
    #[serde(flatten)]
    pub triggers: OpenAiCategoryTriggers,
    #[serde(flatten)]
    pub retry: ApiRetry,
}

#[async_trait]
impl Condition for FlaggedByOmniModeration {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        normalize_api_key(&mut self.api_key, "OpenAI", TITLE)?;
        api_retry::validate(&self.retry, TITLE)?;

        for category in OpenAiCategory::ALL {
            if let CategoryTrigger::MinScorePercent(percent) = self.triggers.get(category)
                && !(1..=100).contains(&percent)
            {
                return Err(format!(
                    "The minimum score for '{}' must be between 1 and 100, got {percent}",
                    category.api_name()
                )
                .into());
            }
        }
        if OpenAiCategory::ALL
            .into_iter()
            .all(|category| self.triggers.get(category) == CategoryTrigger::Off)
        {
            return Err(
                "'Flagged by OpenAI Omni' has every category off, so it can never match".into(),
            );
        }
        Ok(())
    }

    fn describe(&self) -> String {
        "flagged by OpenAI Omni".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        let text = &ctx.group_message.text;
        // Nothing to show OpenAI: a caption-less attachment or blank text.
        if text.trim().is_empty() {
            return Ok(None);
        }
        match ctx.openai.classify(&self.api_key, text, &self.retry).await {
            Ok(verdict) => Ok(filter::should_moderate(&self.triggers, &verdict)),
            // No verdict is no match: OpenAI being down, rate limited or
            // refusing the key must not stop the other rules. The adapter
            // has logged why.
            Err(_) => Ok(None),
        }
    }
}
