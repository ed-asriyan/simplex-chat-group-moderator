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
/// US-D4, US-D6): the rule's name, if it has one, what matched, and whether
/// the rule's actions are on — its line is marked `_(off)_` when they are not.
/// This context's own copy of the moderator's type; the notification router
/// translates between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleReason {
    pub rule_name: Option<String>,
    pub reason: String,
    pub actions_on: bool,
}

/// What made the bot check the rules, with what the owner is shown of it. This
/// context's own copy of the moderator's type; the notification router
/// translates between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggerEvent {
    /// A member's message, quoted from `message` (US-D2).
    MessagePosted { message: String },
    /// Someone joined (US-D6).
    MemberJoined,
}

/// One moderation notification for an owner (US-D2, US-D4, US-D6): what was
/// carried out, then what would have been, for the rules that tell the owner.
/// At least one of the two lists holds an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationNotification {
    pub event: TriggerEvent,
    /// The display name of the message's author, or of the person who joined.
    pub member_name: String,
    /// What was carried out, in the order it was.
    pub performed: Vec<ModerationAction>,
    /// What rules with their actions off would have carried out, in the order
    /// it would have been, none of it covered by `performed`.
    pub would_perform: Vec<ModerationAction>,
    /// One per matched rule, in the order of the rules.
    pub reasons: Vec<RuleReason>,
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

/// Inbound port: deliver a notification to a group owner of the moderation
/// actions the bot carried out in their group, and of those it would have
/// carried out (US-D2, US-D4, US-D6).
#[async_trait]
pub trait ModerationNotificationReceiver: Send + Sync {
    async fn send_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        notification: &ModerationNotification,
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

    /// Replace the group's whole configuration — its rules, each with its
    /// switches — with the one in `json` (US-C1).
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
