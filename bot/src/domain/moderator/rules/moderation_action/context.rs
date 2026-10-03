//! What an action is carried out with, and what it reports back.

use crate::domain::moderator::ports::{Err, GroupMessage, GroupModerator, MemberRestoreRepository};

/// Every port an action may act through. The application builds one from what
/// it was wired with; nothing outside `rules` knows which action uses which.
pub struct ActionPorts<'a> {
    pub group_moderator: &'a dyn GroupModerator,
    pub restores: &'a dyn MemberRestoreRepository,
}

pub(in crate::domain::moderator::rules) struct ActionContext<'a> {
    pub group_message: &'a GroupMessage,
    pub ports: &'a ActionPorts<'a>,
}

/// What an action that was carried out has to say beyond success.
#[derive(Default)]
pub(in crate::domain::moderator::rules) struct ActionReport {
    /// Bookkeeping the bot keeps for itself that failed. It must not call off
    /// the moderation of the message or the owner's notification, so it is
    /// reported here rather than as the action's error.
    pub bookkeeping_error: Option<Err>,
}
