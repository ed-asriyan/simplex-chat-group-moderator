use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::bot_dm::ports::{BotMessenger, Err as BotDmErr, UserId as BotDmUserId};
use crate::domain::moderator::ports::{
    Err as ModeratorErr, GroupId as ModGroupId, GroupModerator, MessageId as ModMessageId,
    MessengerGroupId,
};
use crate::infrastructure::drivers::simplex::SimplexDriver;

/// Adapter that bridges the driver onto the per-domain outbound ports
/// (`bot_dm::BotMessenger` and `moderator::GroupModerator`).
#[derive(Clone)]
pub struct SimplexAdapter {
    driver: Arc<SimplexDriver>,
}

impl SimplexAdapter {
    pub fn new(driver: Arc<SimplexDriver>) -> Self {
        Self { driver }
    }
}

#[async_trait]
impl BotMessenger for SimplexAdapter {
    async fn send_dm(&self, user_id: &BotDmUserId, text: &str) -> Result<(), BotDmErr> {
        self.driver
            .send_message_text(*user_id, text.to_string(), None)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })?;
        Ok(())
    }
}

#[async_trait]
impl GroupModerator for SimplexAdapter {
    async fn delete_message(
        &self,
        group_id: &ModGroupId,
        message_id: &ModMessageId,
    ) -> Result<(), ModeratorErr> {
        self.driver
            .moderate_group_message(*group_id, *message_id)
            .await
            .map_err(|e| -> ModeratorErr { e.to_string().into() })?;
        Ok(())
    }

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, ModeratorErr> {
        self.driver
            .join_group(messenger_group_id)
            .await
            .map_err(|e| -> ModeratorErr { e.to_string().into() })?;
        Ok(messenger_group_id)
    }
}
