//! `ContainsLinksOutsideTop100`: the message links to a domain outside the built-in top 100 and the owner's list.

use super::links;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::{checks, domains};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message links to a domain covered neither by the built-in top-100
/// list nor by `domains`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsLinksOutsideTop100 {
    pub domains: Vec<String>,
}

#[async_trait]
impl Condition for ContainsLinksOutsideTop100 {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::normalize_list(&mut self.domains);
        checks::check_list(
            &self.domains,
            domains::MAX_DOMAIN_LENGTH,
            "domains",
            "Domain",
        )
    }

    fn describe(&self) -> String {
        "contains a link outside the top 100 websites".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(links::should_moderate_outside_top100(
            ctx.group_message.text.trim(),
            &self.domains,
        ))
    }
}
