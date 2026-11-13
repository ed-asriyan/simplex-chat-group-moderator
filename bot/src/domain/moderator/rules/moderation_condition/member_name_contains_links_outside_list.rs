//! `MemberNameContainsLinksOutsideList`: the member's name links to a domain
//! the owner's list does not cover, seen through disguises as a message's
//! links are (US-E32).
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

/// The member's name links to a domain not covered by `domains`. An empty
/// list makes every link match (US-E32).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemberNameContainsLinksOutsideList {
    pub domains: Vec<String>,
}

impl Condition for MemberNameContainsLinksOutsideList {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        todo!()
    }

    fn describe(&self) -> String {
        todo!()
    }
}

#[async_trait]
impl<S: Member + ?Sized> ConditionOn<S> for MemberNameContainsLinksOutsideList {
    async fn should_moderate(&self, ctx: &ConditionContext<'_, S>) -> Result<Option<String>, Err> {
        let _ = ctx;
        todo!()
    }
}
