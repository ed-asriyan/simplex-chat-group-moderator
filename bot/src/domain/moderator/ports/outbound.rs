//! Outbound (driven) ports: what this context needs from the outside.
//!
//! Implemented by adapters in `infrastructure/adapters/`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use super::types::{
    Err, Group, GroupId, GroupMemberRole, MessageId, MessengerGroupId, ModerationAction,
    ModerationRule, OwnedModerationRule, ScheduledMemberRestore, UserId,
};

/// Outbound port: notify a group owner that moderation actions were performed.
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
    /// Register a new group and return the generated `group_id`.
    async fn save_owner(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
        owner_id: &UserId,
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

    async fn get_group_rules(&self, group_id: &GroupId) -> Result<Vec<OwnedModerationRule>, Err>;

    async fn get_group_rules_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err>;

    async fn set_group_rules(
        &self,
        group_id: &GroupId,
        rules: &[ModerationRule],
    ) -> Result<(), Err>;

    async fn delete_group_data(&self, messenger_group_id: &MessengerGroupId) -> Result<(), Err>;

    async fn get_group_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<Group>, Err>;

    async fn set_notifications_enabled(&self, group_id: &GroupId, enabled: bool)
    -> Result<(), Err>;

    async fn set_dry_mode_enabled(&self, group_id: &GroupId, enabled: bool) -> Result<(), Err>;
}

/// Outbound port: persistence for user activity and rate limit state.
#[async_trait]
pub trait UserActivityRepository: Send + Sync {
    async fn record_user_message(
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
