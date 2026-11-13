//! What a leaf condition is checked with: the trigger's subject and every port
//! a condition may read.
//!
//! The ports are bundled rather than threaded through as parameters, because
//! the composites make evaluation recursive. What the evaluation keeps for
//! itself — the per-message memo that checks every distinct condition at most
//! once, and the pre-pass flag — is the engine's, not a leaf's, and is not
//! here.

use crate::domain::moderator::ports::{
    GroupCharacterActivityRepository, GroupLineActivityRepository, GroupMessageActivityRepository,
    GroupMessageHistoryRepository, OpenAi, OpenRouter, UserCharacterActivityRepository,
    UserLineActivityRepository, UserMessageActivityRepository, UserModerationActivityRepository,
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

/// What one leaf is checked with. `S` is the trigger's subject: the
/// `GroupMessage` for a message, the `MemberJoin` for a join.
pub(in crate::domain::moderator::rules) struct ConditionContext<'a, S: ?Sized> {
    pub subject: &'a S,
    pub ports: &'a ConditionPorts<'a>,

    /// Whether this message is moderated by a rule that does not itself
    /// depend on the other rules. Computed by the pre-pass, for the conditions
    /// that do depend on them to read; always false for a join.
    pub moderated_by_other_rules: bool,
}
