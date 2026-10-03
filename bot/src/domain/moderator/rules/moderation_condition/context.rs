//! Everything one message's rule evaluation needs, carried in one place.
//!
//! The repositories used to be threaded through every call as separate
//! parameters. With conditions forming a tree those calls are recursive, so the
//! parameters are bundled here instead. The context also owns the per-message
//! memo, which is what keeps the old guarantee that every distinct condition is
//! evaluated at most once per message: with trees, identical subtrees appear
//! across rules as soon as owners copy a rule and edit one branch.

use std::collections::HashMap;

use super::ModerationCondition;
use crate::domain::moderator::ports::{
    GroupCharacterActivityRepository, GroupLineActivityRepository, GroupMessage,
    GroupMessageActivityRepository, GroupMessageHistoryRepository, OpenAi, OpenRouter,
    UserCharacterActivityRepository, UserLineActivityRepository, UserMessageActivityRepository,
    UserModerationActivityRepository,
};

/// Every port a condition may read, and `bookkeeping` may record into. The
/// application builds one from what it was wired with; nothing outside
/// `rules` knows which condition reads which.
pub struct ConditionPorts<'a> {
    pub activity_repo: &'a dyn UserMessageActivityRepository,
    pub character_activity_repo: &'a dyn UserCharacterActivityRepository,
    pub line_activity_repo: &'a dyn UserLineActivityRepository,
    pub moderation_activity_repo: &'a dyn UserModerationActivityRepository,
    pub group_activity_repo: &'a dyn GroupMessageActivityRepository,
    pub group_character_activity_repo: &'a dyn GroupCharacterActivityRepository,
    pub group_line_activity_repo: &'a dyn GroupLineActivityRepository,
    pub message_history: &'a dyn GroupMessageHistoryRepository,
    pub openai: &'a dyn OpenAi,
    pub openrouter: &'a dyn OpenRouter,
}

pub(in crate::domain::moderator::rules) struct ConditionContext<'a> {
    pub group_message: &'a GroupMessage,
    pub ports: &'a ConditionPorts<'a>,

    /// Whether this message is moderated by a rule that does not itself
    /// depend on the other rules. Computed by the pre-pass, for the conditions
    /// that do depend on them to read.
    pub moderated_by_other_rules: bool,

    /// While set, every condition that depends on the other rules evaluates to
    /// "no match". The pre-pass uses this to answer "is this message moderated
    /// by anything else" without asking such a condition about itself.
    pub pre_pass: bool,

    /// Results of conditions already evaluated for this message. Only subtrees
    /// that do not depend on the other rules are stored, since those are the
    /// only ones whose result does not depend on `pre_pass`.
    pub memo: HashMap<ModerationCondition, Option<String>>,
}

impl<'a> ConditionContext<'a> {
    pub fn new(group_message: &'a GroupMessage, ports: &'a ConditionPorts<'a>) -> Self {
        Self {
            group_message,
            ports,
            moderated_by_other_rules: false,
            pre_pass: false,
            memo: HashMap::new(),
        }
    }
}
