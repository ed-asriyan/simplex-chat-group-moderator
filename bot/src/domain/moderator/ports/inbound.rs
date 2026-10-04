//! Inbound (driving) ports: how the outside world drives this context.
//!
//! One trait per use case, each implemented by its own application in
//! `application/`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::types::{
    Err, Group, GroupConfig, GroupId, GroupMessage, MessengerGroup, MessengerGroupId, UserId,
};

/// Inbound port: moderate a message posted in a group the bot has joined.
#[async_trait]
pub trait ModerationEngine: Send + Sync {
    async fn process_group_message(&self, group_message: GroupMessage) -> Result<(), Err>;
}

/// Inbound port: restore the members whose observer time is up by `now`.
///
/// Driven by a clock rather than by a messenger event, so the caller supplies
/// the current time and the domain stays free of one.
#[async_trait]
pub trait MemberRestoreRunner: Send + Sync {
    async fn run_due_restores(&self, now: DateTime<Utc>) -> Result<(), Err>;
}

/// Inbound port: manage an owner's groups and their moderation configuration.
#[async_trait]
pub trait GroupAdministration: Send + Sync {
    async fn try_join_group(&self, owner_id: UserId, group: &MessengerGroup) -> Result<Group, Err>;

    async fn remove_group(&self, messenger_group_id: MessengerGroupId) -> Result<(), Err>;

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err>;

    /// The group's whole configuration, as the owner last saved it.
    async fn get_group_config(
        &self,
        user_id: UserId,
        group_id: GroupId,
    ) -> Result<GroupConfig, Err>;

    /// Replace the group's whole configuration at once: nothing of it is
    /// stored unless all of it is valid.
    async fn set_group_config(
        &self,
        user_id: UserId,
        group_id: GroupId,
        config: GroupConfig,
    ) -> Result<(), Err>;
}
