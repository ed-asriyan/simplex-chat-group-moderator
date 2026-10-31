//! Outbound (driven) ports: what this context needs from the outside.

use async_trait::async_trait;

use super::types::{Err, Group, GroupId, GroupInvitation, JoinError, UserId};

/// Outbound port: send direct messages back to a user.
#[async_trait]
pub trait BotMessenger: Send + Sync {
    async fn send_dm(&self, user_id: &UserId, text: &str) -> Result<(), Err>;
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

    async fn get_groups(&self, user_id: UserId) -> Result<Vec<Group>, Err>;

    /// Replace the group's whole configuration — its mode and its rules —
    /// with the one in `json`.
    async fn set_config_json(
        &self,
        user_id: UserId,
        group_id: GroupId,
        json: &str,
    ) -> Result<(), Err>;

    /// The group's whole configuration as JSON.
    async fn get_config_json(&self, user_id: UserId, group_id: GroupId) -> Result<String, Err>;
}
