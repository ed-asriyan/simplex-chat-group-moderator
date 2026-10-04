//! `ContainsRepeatedSequence`: some run of characters repeats back to back.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Some sequence of at least `min_length` characters appears at least
/// `min_repeats` times in a row.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsRepeatedSequence {
    pub min_repeats: u32,
    pub min_length: u32,
}

#[async_trait]
impl Condition for ContainsRepeatedSequence {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        // A single occurrence is not a repetition: 1 (or 0) would match
        // every non-empty message.
        if self.min_repeats < 2 {
            return Err(format!(
                "Minimum repeats must be at least 2, got {}",
                self.min_repeats
            )
            .into());
        }
        if !(1..=filter::MAX_SEQUENCE_LENGTH).contains(&self.min_length) {
            return Err(format!(
                "Minimum sequence length must be between 1 and {}, got {}",
                filter::MAX_SEQUENCE_LENGTH,
                self.min_length
            )
            .into());
        }
        Ok(())
    }

    fn describe(&self) -> String {
        "contains a repeated sequence".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(filter::should_moderate(
            &ctx.group_message.text,
            self.min_repeats,
            self.min_length,
        ))
    }
}
