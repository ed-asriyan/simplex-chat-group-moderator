//! `MemberNameContainsWords`: the member's name contains any of the owner's
//! words, seen through obfuscation as a message's text is (US-E31).
//!
//! The name is the display name and the full name of the member's profile: the
//! author's in a message rule, the newcomer's in a join rule, which is why it
//! works with both triggers.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext, ConditionOn};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::member::Member;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The member's name contains any of `keywords` as a whole word or phrase,
/// seen through obfuscation (US-E31).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemberNameContainsWords {
    pub keywords: Vec<String>,
}

impl Condition for MemberNameContainsWords {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        todo!()
    }

    fn describe(&self) -> String {
        todo!()
    }
}

#[async_trait]
impl<S: Member + ?Sized> ConditionOn<S> for MemberNameContainsWords {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, S>) -> Result<Option<String>, Err> {
        let _ = ctx;
        todo!()
    }
}
