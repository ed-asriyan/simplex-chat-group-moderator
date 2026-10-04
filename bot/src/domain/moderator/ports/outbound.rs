//! Outbound (driven) ports: what this context needs from the outside.
//!
//! Implemented by adapters in `infrastructure/adapters/`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use super::types::{
    ApiRetry, Err, Group, GroupConfig, GroupId, GroupMemberRole, GroupMode,
    InstructionContextMessage, KeyCheck, MessageId, MessengerGroupId, ModerationAction,
    OpenAiModerationResult, OpenRouterInstructionVerdict, OwnedModerationRule, RecentGroupMessage,
    ScheduledMemberRestore, UserId,
};

/// Outbound port: notify a group owner of the actions a message called for —
/// carried out, or only reported, as the group's mode says.
#[async_trait]
pub trait ModerationNotifier: Send + Sync {
    async fn notify_moderation_action(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        message: &str,
        reasons: &[String],
    ) -> Result<(), Err>;
}

/// Outbound port: actions the moderator performs in a group.
#[async_trait]
pub trait GroupModerator: Send + Sync {
    async fn delete_message(&self, group_id: &GroupId, message_id: &MessageId) -> Result<(), Err>;

    async fn kick_member(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        delete_all_messages: bool,
    ) -> Result<(), Err>;

    async fn set_member_role(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        role: GroupMemberRole,
    ) -> Result<(), Err>;

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, Err>;
}

/// Outbound port: persistence for the member restores the bot still owes.
///
/// Plain storage: when a restore replaces, cancels or completes another one is
/// decided by the use cases, not here.
#[async_trait]
pub trait MemberRestoreRepository: Send + Sync {
    /// Stores the restore due for this member, replacing the one already stored
    /// for them, if any.
    async fn save(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
        execute_at: DateTime<Utc>,
    ) -> Result<(), Err>;

    async fn delete_for_member(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
    ) -> Result<(), Err>;

    async fn list_due(&self, now: DateTime<Utc>) -> Result<Vec<ScheduledMemberRestore>, Err>;

    /// Removes the restore only while it still carries `execute_at`. `save`
    /// reuses a member's row, so the deadline is what tells a restore apart from
    /// the one that replaced it.
    async fn delete(&self, id: i64, execute_at: DateTime<Utc>) -> Result<(), Err>;
}

/// Outbound port: persistence for moderator state.
#[async_trait]
pub trait ModerationRepository: Send + Sync {
    /// Register a new group in `mode` and return the generated `group_id`.
    async fn save_owner(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
        owner_id: &UserId,
        mode: GroupMode,
    ) -> Result<GroupId, Err>;

    async fn get_owner_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<UserId>, Err>;

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err>;

    async fn get_owner_by_id(&self, group_id: &GroupId) -> Result<Option<UserId>, Err>;

    async fn set_group_name(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
    ) -> Result<(), Err>;

    /// The group's mode and rules. The group must be registered.
    async fn get_group_config(&self, group_id: &GroupId) -> Result<GroupConfig, Err>;

    async fn get_group_rules_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err>;

    /// Replace the group's mode and rules in one transaction.
    async fn set_group_config(&self, group_id: &GroupId, config: &GroupConfig) -> Result<(), Err>;

    async fn delete_group_data(&self, messenger_group_id: &MessengerGroupId) -> Result<(), Err>;

    async fn get_group_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<Group>, Err>;
}

/// Outbound port: persistence for user activity and rate limit state.
#[async_trait]
pub trait UserMessageActivityRepository: Send + Sync {
    async fn record_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn count_messages_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: persistence for how much a user wrote and the character rate
/// limit state.
///
/// Separate from [`UserMessageActivityRepository`] because it answers a
/// different question about the same traffic: not how often a member posts, but
/// how much they write. A group that configures only one of the two pays for
/// only one.
#[async_trait]
pub trait UserCharacterActivityRepository: Send + Sync {
    async fn record_characters(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        character_count: u32,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn sum_characters_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: persistence for how many lines a user took up on screen and
/// the line rate limit state.
///
/// Lines are counted by the caller, with the wrap width configured at the time
/// the message arrived; this port only keeps the sum over the window. A message
/// already counted is never recounted, so changing the width affects new
/// messages only.
#[async_trait]
pub trait UserLineActivityRepository: Send + Sync {
    async fn record_lines(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        line_count: u32,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn sum_lines_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: how many messages a whole group received recently, from all
/// its members together — the group-wide counterpart of
/// [`UserMessageActivityRepository`].
///
/// A storage of its own rather than a sum over the members' counters: those
/// are kept only while some rule limits authors, and for that rule's window,
/// not this one's.
#[async_trait]
pub trait GroupMessageActivityRepository: Send + Sync {
    async fn record_message(
        &self,
        group_id: &MessengerGroupId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn count_messages_since(
        &self,
        group_id: &MessengerGroupId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: how many characters a whole group's members wrote recently,
/// all together — the group-wide counterpart of
/// [`UserCharacterActivityRepository`].
#[async_trait]
pub trait GroupCharacterActivityRepository: Send + Sync {
    async fn record_characters(
        &self,
        group_id: &MessengerGroupId,
        timestamp: DateTime<Utc>,
        character_count: u32,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn sum_characters_since(
        &self,
        group_id: &MessengerGroupId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: how many lines a whole group's messages took on screen
/// recently — the group-wide counterpart of [`UserLineActivityRepository`].
/// Lines are counted by the caller, with the width the group-wide conditions
/// configured when the message arrived.
#[async_trait]
pub trait GroupLineActivityRepository: Send + Sync {
    async fn record_lines(
        &self,
        group_id: &MessengerGroupId,
        timestamp: DateTime<Utc>,
        line_count: u32,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn sum_lines_since(
        &self,
        group_id: &MessengerGroupId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: persistence for user moderation activity (moderated messages history) and moderation rate limit state.
#[async_trait]
pub trait UserModerationActivityRepository: Send + Sync {
    async fn record_moderated_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err>;

    async fn count_moderated_messages_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err>;
}

/// Outbound port: a group's latest messages, kept for
/// `FlaggedByOpenRouterInstruction` to send along as context — the messenger
/// is not asked for its history.
///
/// Only messages still standing in the chat are kept: the caller records a
/// message once it has been moderated and was not deleted, and forgets an
/// edited one the bot deleted. `keep` is how many of a group's latest
/// messages the rules need right now; the adapter caps how long it keeps any.
#[async_trait]
pub trait GroupMessageHistoryRepository: Send + Sync {
    /// Keep `message` as the group's latest, dropping the oldest beyond `keep`.
    async fn record_message(
        &self,
        group_id: &MessengerGroupId,
        message: RecentGroupMessage,
        keep: u32,
    ) -> Result<(), Err>;

    /// Replace a kept message with its edit, in place. A message no longer
    /// kept stays forgotten.
    async fn record_edit(
        &self,
        group_id: &MessengerGroupId,
        message: RecentGroupMessage,
    ) -> Result<(), Err>;

    /// Forget a kept message: the bot deleted it from the chat.
    async fn forget_message(
        &self,
        group_id: &MessengerGroupId,
        message_id: &MessageId,
    ) -> Result<(), Err>;

    /// Up to `count` kept messages that came before `message_id`, oldest
    /// first — the latest `count` when `message_id` is not kept, which is the
    /// case for a new message, since it is recorded only once moderated.
    async fn messages_before(
        &self,
        group_id: &MessengerGroupId,
        message_id: &MessageId,
        count: u32,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecentGroupMessage>, Err>;
}

/// Outbound port: what the moderator asks OpenAI, always with the owner's own
/// key: verdicts from the moderation model.
///
/// An `Err` from `classify` means there is no verdict — OpenAI was
/// unreachable, refused the key, rate limited it, or the request waited too
/// long — and callers treat it as "no match": a failing provider must not stop
/// the rest of moderation.
///
/// `verify` is asked when an owner saves rules.
#[async_trait]
pub trait OpenAi: Send + Sync {
    async fn classify(
        &self,
        api_key: &str,
        text: &str,
        retry: &ApiRetry,
    ) -> Result<OpenAiModerationResult, Err>;

    /// Whether a key can call the moderation endpoint.
    async fn verify(&self, api_key: &str) -> KeyCheck;
}

/// Outbound port: what the moderator asks OpenRouter, always with the owner's
/// own key: verdicts from a model following the owner's instruction.
///
/// An `Err` from `matches_instruction` means there is no verdict, exactly as
/// for [`OpenAi::classify`], and callers treat it as "no match".
///
/// `verify_model` is asked when an owner saves rules.
#[async_trait]
pub trait OpenRouter: Send + Sync {
    /// Whether `model`, given the owner's `instruction`, says `text`, written
    /// by `author_name`, is what the instruction describes, and why. `context`
    /// is the group's earlier messages, oldest first, for the model to read but
    /// not judge.
    async fn matches_instruction(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        author_name: &str,
        text: &str,
        context: &[InstructionContextMessage],
        retry: &ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err>;

    /// Whether a key can have `model` answer — a real request, a few tokens
    /// long, so it also proves the account has credits.
    async fn verify_model(&self, api_key: &str, model: &str) -> KeyCheck;
}
