//! Types the moderator context exchanges across its ports.

pub use crate::domain::moderator::message_filter::{
    ApiRetry, CategoryTrigger, ModerationAction, ModerationCondition, ModerationMatch,
    ModerationRule, OpenAiCategory, OpenAiCategoryTriggers, OpenAiModerationResult,
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

/// What a message carries besides its text. The messenger sends one attachment
/// per message, with the caption as the message text, so this is one kind and
/// not a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAttachment {
    Image,
    Video,
    Voice,
    File,
}

#[derive(Clone, Debug, Default)]
pub struct GroupMessage {
    pub group: MessengerGroup,
    pub message_id: MessageId,
    pub author_id: UserId,
    pub text: String,
    /// What the message carries besides its text; `None` for a plain text
    /// message. An attachment's caption is the `text` above, so a picture with
    /// no caption is a message with an attachment and blank text.
    pub attachment: Option<MessageAttachment>,
    pub timestamp: DateTime<Utc>,
    /// When the author joined the group, if known. Only members who joined
    /// after the bot have a known join time; for everyone who was already there
    /// when the bot joined this is `None`.
    pub author_joined_at: Option<DateTime<Utc>>,
    /// An edit of a message already posted, rather than a new message. It is
    /// moderated like any other message — an edit into a banned word is still
    /// caught — but it is not new traffic, so it feeds no rate limit counter.
    /// `timestamp` is then when the edit was made.
    pub is_edit: bool,
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

/// What the provider — OpenAI or OpenRouter — said about an API key an owner
/// is saving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCheck {
    /// The key can make the request the condition will make.
    Valid,
    /// The provider does not know the key (401): mistyped, revoked or
    /// disabled.
    Rejected,
    /// The key exists but may not make that request (403): an OpenAI key
    /// without the Moderations permission, or an OpenRouter key whose
    /// guardrail does not allow the model.
    Forbidden,
    /// The account behind the key cannot pay (OpenAI: 429
    /// `insufficient_quota`; OpenRouter: 402, or the key's own credit limit is
    /// spent). Retrying does not help.
    QuotaExceeded,
    /// The model cannot be asked this way (400/404): a model the provider
    /// retired, or one that has no endpoint left taking the request.
    ModelUnavailable,
    /// The provider could not be asked: network trouble, 5xx, rate limiting,
    /// or an answer that could not be read. Says nothing about the key itself.
    Unreachable,
}

/// A model's answer about one message: whether it is what the owner's
/// instruction describes, and the model's own one-sentence reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRouterInstructionVerdict {
    pub matches: bool,
    pub reason: String,
}
