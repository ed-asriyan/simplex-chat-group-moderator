//! `ContainsLinksOutsideTop100`: the message links to a domain outside the built-in top 100 and the owner's list.

#[cfg(test)]
mod tests;
mod top100;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::domains::{domain_matches, find_domains};
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
        Ok(should_moderate_outside_top100(
            ctx.group_message.text.trim(),
            &self.domains,
        ))
    }
}

/// Returns `Some(domain)` if `text` contains a link whose domain is neither in
/// the built-in top-100 list nor in `extra`.  Messages with no links never
/// match.
fn should_moderate_outside_top100(text: &str, extra: &[String]) -> Option<String> {
    let domains = find_domains(text);
    for domain in &domains {
        let is_listed = top100::DOMAINS
            .iter()
            .any(|pattern| domain_matches(domain, pattern))
            || extra.iter().any(|pattern| domain_matches(domain, pattern));
        if !is_listed {
            return Some(domain.clone());
        }
    }
    None
}
