//! Everything one message's rule evaluation needs, carried in one place.
//!
//! The repositories used to be threaded through every call as separate
//! parameters. With conditions forming a tree those calls are recursive, so the
//! parameters are bundled here instead. The struct also owns the per-message
//! memo, which is what keeps the old guarantee that every distinct condition is
//! evaluated at most once per message: with trees, identical subtrees appear
//! across rules as soon as owners copy a rule and edit one branch.

use std::collections::HashMap;

use super::ModerationCondition;
use crate::domain::moderator::ports::{
    GroupMessage, UserActivityRepository, UserModerationActivityRepository,
};

pub(in crate::domain::moderator::message_filter) struct ConditionContext<'a> {
    pub group_message: &'a GroupMessage,
    pub activity_repo: &'a dyn UserActivityRepository,
    pub moderation_activity_repo: &'a dyn UserModerationActivityRepository,

    /// Whether this message is moderated by a rule that does not itself depend
    /// on the moderation rate limit. Computed by the pre-pass and read by
    /// `UserExceedsModerationRateLimit` so it can count the current message.
    pub message_is_moderated: bool,

    /// While set, every `UserExceedsModerationRateLimit` evaluates to "no
    /// match". The pre-pass uses this to answer "is this message moderated by
    /// anything else" without asking the rate limit about itself.
    pub moderation_rate_limit_pinned: bool,

    /// Results of conditions already evaluated for this message. Only subtrees
    /// free of `UserExceedsModerationRateLimit` are stored, since those are the
    /// only ones whose result does not depend on `moderation_rate_limit_pinned`.
    pub memo: HashMap<ModerationCondition, Option<String>>,
}

impl<'a> ConditionContext<'a> {
    pub fn new(
        group_message: &'a GroupMessage,
        activity_repo: &'a dyn UserActivityRepository,
        moderation_activity_repo: &'a dyn UserModerationActivityRepository,
    ) -> Self {
        Self {
            group_message,
            activity_repo,
            moderation_activity_repo,
            message_is_moderated: false,
            moderation_rate_limit_pinned: false,
            memo: HashMap::new(),
        }
    }
}
