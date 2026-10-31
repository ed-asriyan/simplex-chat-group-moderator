//! Inbound (driving) ports: how the outside world drives this context.

use async_trait::async_trait;

use super::types::{Err, Group, GroupInvitation, Message, ModerationAction, UserId};

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

/// Inbound port: deliver a notification to a group owner that the bot performed
/// moderation actions in their group — or, when `performed` is false, would
/// have.
#[async_trait]
pub trait ModerationNotificationReceiver: Send + Sync {
    async fn send_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        performed: bool,
        message: &str,
        reasons: &[String],
    ) -> Result<(), Err>;
}
