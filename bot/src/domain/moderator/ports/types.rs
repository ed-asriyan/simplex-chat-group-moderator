//! Types the moderator context exchanges across its ports.

pub use crate::domain::moderator::message_filter::{
    ModerationAction, ModerationCondition, ModerationMatch, ModerationRule,
};
use chrono::{DateTime, Utc};
use std::error::Error;

pub type MessengerGroupId = i64;
pub type GroupId = i64;
pub type MessageId = i64;
pub type UserId = i64;

pub type Err = Box<dyn Error + Send + Sync>;

#[derive(Clone, Debug)]
pub struct Group {
    pub id: GroupId,
    pub owner_id: UserId,
    pub name: String,
    pub notifications_enabled: bool,
    pub dry_mode_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MessengerGroup {
    pub id: MessengerGroupId,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct GroupInvitation {
    pub group: MessengerGroup,
    pub is_moderator: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GroupMessage {
    pub group: MessengerGroup,
    pub message_id: MessageId,
    pub author_id: UserId,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    /// When the author joined the group, if known. Only members who joined
    /// after the bot have a known join time; for everyone who was already there
    /// when the bot joined this is `None`.
    pub author_joined_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct OwnedModerationRule {
    pub id: usize,
    pub rule: ModerationRule,
}

/// The roles the bot moves a member between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMemberRole {
    Member,
    Observer,
}

/// A member the bot still owes a restore to: it made them an observer for a
/// limited time, and at `execute_at` that time is up.
#[derive(Clone, Debug)]
pub struct ScheduledMemberRestore {
    pub id: i64,
    pub messenger_group_id: MessengerGroupId,
    pub member_id: UserId,
    pub execute_at: DateTime<Utc>,
}
