//! The three link conditions, on top of the domain extraction they share in
//! `common::domains`.

use super::top100;
use crate::domain::moderator::rules::common::domains::{
    domain_matches, find_domains, find_in_list, find_outside_list,
};

/// Returns `Some(domain)` if `text` contains a link whose domain is in `list`,
/// or `None` if no link is (or there are no links).
pub fn should_moderate_in_list(text: &str, list: &[String]) -> Option<String> {
    find_in_list(&find_domains(text), list)
}

/// Returns `Some(domain)` if `text` contains a link whose domain is **not**
/// covered by `list`, or `None` if every link is (or there are no links at all).
pub fn should_moderate_outside_list(text: &str, list: &[String]) -> Option<String> {
    find_outside_list(&find_domains(text), list)
}

/// Returns `Some(domain)` if `text` contains a link whose domain is neither in
/// the built-in top-100 list nor in `extra`.  Messages with no links never
/// match.
pub fn should_moderate_outside_top100(text: &str, extra: &[String]) -> Option<String> {
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
