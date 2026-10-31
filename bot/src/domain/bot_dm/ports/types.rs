//! The value types `bot_dm`'s ports exchange.

use std::error::Error;

pub type GroupId = i64;
pub type MessageId = i64;
pub type UserId = i64;

pub type Err = Box<dyn Error + Send + Sync>;

pub type JoinError = Box<dyn Error + Send + Sync>;

/// One action the moderator reported having performed. This context's own copy
/// of the moderator's action type; the notification router translates between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModerationAction {
    ModerateMessage,
    SetAuthorObserver { duration_minutes: u32 },
    KickAuthor { delete_all_messages: bool },
}

#[derive(Debug)]
pub struct Message {
    pub text: String,
    pub reply_to_message: Option<String>,
}

pub struct Group {
    pub id: GroupId,
    pub name: String,
}

/// An invitation to a group the bot has not joined yet: it has no settings of
/// its own until the moderator registers it.
pub struct GroupInvitation {
    /// The messenger's id of the group.
    pub group_id: GroupId,
    pub group_name: String,
    /// Whether the bot was invited as a moderator or higher; it cannot delete
    /// messages with less, so it does not join.
    pub is_moderator_or_higher: bool,
}
