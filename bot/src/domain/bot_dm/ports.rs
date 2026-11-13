use async_trait::async_trait;
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

/// Why one rule matched, as the owner reads it in a notification (US-D2,
/// US-D6): the rule's name, if it has one, and what matched. This context's own
/// copy of the moderator's type; the notification router translates between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleReason {
    pub rule_name: Option<String>,
    pub reason: String,
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

/// Inbound port: entry point for direct messages addressed to the bot.
#[async_trait]
pub trait BotDmReceiver: Send + Sync {
    async fn handle_dm(&self, user_id: UserId, message: &Message) -> Result<(), Err>;

    async fn handle_group_invitation(
        &self,
        user_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<(), Err>;
}

/// Outbound port: send direct messages back to a user.
#[async_trait]
pub trait BotMessenger: Send + Sync {
    async fn send_dm(&self, user_id: &UserId, text: &str) -> Result<(), Err>;
}

/// Inbound port: deliver a notification to a group owner that the bot performed
/// moderation actions in their group — or, when `performed` is false, would
/// have. `actions` are in the order they were carried out, `reasons` in the
/// order of the rules, one per matched rule.
#[async_trait]
pub trait ModerationNotificationReceiver: Send + Sync {
    /// About a member's message, quoted from `message` (US-D2, US-D4).
    async fn send_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        performed: bool,
        message: &str,
        reasons: &[RuleReason],
    ) -> Result<(), Err>;

    /// About someone who joined, named by their display name `member_name`
    /// (US-D6).
    async fn send_join_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        performed: bool,
        member_name: &str,
        reasons: &[RuleReason],
    ) -> Result<(), Err>;
}

/// Outbound port: group lifecycle operations the bot triggers from DMs.
#[async_trait]
pub trait GroupOperations: Send + Sync {
    /// Attempt to join the group identified by the given invitation
    async fn try_join_group(
        &self,
        user_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<Group, JoinError>;

    /// The groups the user owns, each with its current name, which the
    /// editor link carries (US-B4).
    async fn get_groups(&self, user_id: UserId) -> Result<Vec<Group>, Err>;

    /// Replace the group's whole configuration — its mode and its rules —
    /// with the one in `json`.
    async fn set_config_json(
        &self,
        user_id: UserId,
        group_id: GroupId,
        json: &str,
    ) -> Result<(), Err>;

    /// The group's whole configuration as JSON, its rules that are off
    /// included (US-B4).
    async fn get_config_json(&self, user_id: UserId, group_id: GroupId) -> Result<String, Err>;
}
