use async_trait::async_trait;
use std::sync::{Arc, OnceLock};

use crate::domain::bot_dm::ports::{Group as BotDmGroup, ModerationNotificationReceiver};
use crate::domain::moderator::ports::{
    Err as ModErr, Group as ModGroup, ModerationNotifier, UserId as ModUserId,
};

/// Bridges the `moderator` bounded context to the `bot_dm` bounded context by
/// implementing `moderator::ModerationNotifier` on top of
/// `bot_dm::ModerationNotificationReceiver`.
///
/// The receiver is injected after construction via [`Self::set_receiver`] to
/// break the wiring cycle between the two bounded contexts (the moderator app
/// depends on this notifier, while the bot_dm app that fulfils the receiver port
/// depends — through `GroupOperations` — on the moderator app).
#[derive(Default)]
pub struct ModerationNotificationRouter {
    receiver: OnceLock<Arc<dyn ModerationNotificationReceiver>>,
}

impl ModerationNotificationRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wire the inbound `bot_dm` receiver. Subsequent calls are ignored.
    pub fn set_receiver(&self, receiver: Arc<dyn ModerationNotificationReceiver>) {
        let _ = self.receiver.set(receiver);
    }
}

#[async_trait]
impl ModerationNotifier for ModerationNotificationRouter {
    async fn notify_moderated_message(
        &self,
        user_id: ModUserId,
        group: &ModGroup,
        message: &str,
        phrase: &str,
    ) -> Result<(), ModErr> {
        let receiver = self
            .receiver
            .get()
            .ok_or("moderation notification receiver is not initialized")?;
        let bot_dm_group = BotDmGroup {
            id: group.id,
            name: group.name.clone(),
            notifications_enabled: group.notifications_enabled,
            dry_mode_enabled: group.dry_mode_enabled,
        };
        receiver
            .send_moderation_notification(user_id, &bot_dm_group, message, phrase)
            .await
            .map_err(|e| -> ModErr { e.to_string().into() })
    }
}
