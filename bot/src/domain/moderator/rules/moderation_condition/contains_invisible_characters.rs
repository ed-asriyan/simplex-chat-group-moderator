//! `ContainsInvisibleCharacters`: the message hides at least one invisible
//! character.

mod filter;

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message contains at least one invisible character.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsInvisibleCharacters {}

#[async_trait]
impl Condition for ContainsInvisibleCharacters {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        Ok(())
    }

    fn describe(&self) -> String {
        "contains invisible characters".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        // The shape conditions see the raw message: leading and trailing
        // whitespace is exactly what they are measuring.
        Ok(filter::should_moderate_invisible(
            &ctx.group_message.text,
        ))
    }
}
