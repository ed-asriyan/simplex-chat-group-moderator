//! `MemberNameContainsWords`: the member's name contains any of the owner's
//! words, seen through obfuscation as a message's text is (US-E31).
//!
//! The name is the display name and the full name of the member's profile: the
//! author's in a message rule, the newcomer's in a join rule, which is why it
//! works with both triggers.

use serde::{Deserialize, Serialize};

/// The member's name contains any of `keywords` as a whole word or phrase,
/// seen through obfuscation (US-E31).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemberNameContainsWords {
    pub keywords: Vec<String>,
}
