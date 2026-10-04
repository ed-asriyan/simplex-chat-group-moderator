//! `ContainsLinksInList`: the message links to a domain on the owner's list.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::domains::{find_domains, find_in_list};
use crate::domain::moderator::rules::common::{checks, domains};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message links to a domain covered by `domains`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainsLinksInList {
    pub domains: Vec<String>,
}

#[async_trait]
impl Condition for ContainsLinksInList {
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
        "contains a link to a listed website".into()
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(should_moderate_in_list(
            ctx.group_message.text.trim(),
            &self.domains,
        ))
    }
}

/// Returns `Some(domain)` if `text` contains a link whose domain is in `list`,
/// or `None` if no link is (or there are no links).
fn should_moderate_in_list(text: &str, list: &[String]) -> Option<String> {
    find_in_list(&find_domains(text), list)
}
