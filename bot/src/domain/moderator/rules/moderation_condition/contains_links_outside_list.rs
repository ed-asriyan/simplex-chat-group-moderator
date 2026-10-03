//! `ContainsLinksOutsideList`: the message links to a domain the owner's list does not cover.

use super::links;
use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::{checks, domains};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message links to a domain not covered by `domains`. An empty list
/// makes every link match.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsLinksOutsideList {
    pub domains: Vec<String>,
}

#[async_trait]
impl Condition for ContainsLinksOutsideList {
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
        "contains a link to a website outside the list".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(links::should_moderate_outside_list(
            ctx.group_message.text.trim(),
            &self.domains,
        ))
    }
}
