use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::bot_dm::ports::{
    Err as BotDmErr, Group, GroupId as BotDmGroupId, GroupInvitation as BotDmGroupInvitation,
    GroupOperations, JoinError, UserId as BotDmUserId,
};
use crate::domain::moderator::ports::{
    GroupInvitation as ModGroupInvitation, MessengerGroup, ModerationEngine,
};

/// Bridges the `bot_dm` bounded context to the `moderator` bounded context by
/// implementing `bot_dm::GroupOperations` on top of `moderator::ModerationEngine`.
pub struct CrossDomainRouter {
    moderator: Arc<dyn ModerationEngine>,
}

impl CrossDomainRouter {
    pub fn new(moderator: Arc<dyn ModerationEngine>) -> Self {
        Self { moderator }
    }
}

#[async_trait]
impl GroupOperations for CrossDomainRouter {
    async fn try_join_group(
        &self,
        user_id: BotDmUserId,
        invitation: &BotDmGroupInvitation,
    ) -> Result<Group, JoinError> {
        let mod_invitation = ModGroupInvitation {
            group: MessengerGroup {
                id: invitation.group.id,
                name: invitation.group.name.clone(),
            },
            is_moderator: invitation.is_moderator,
        };
        let group_id = self
            .moderator
            .try_join_group(user_id, &mod_invitation)
            .await
            .map_err(|e| -> JoinError { e.to_string().into() })?;
        Ok(Group {
            id: group_id,
            name: invitation.group.name.clone(),
            notifications_enabled: false,
            dry_mode_enabled: false,
        })
    }

    async fn set_keywords(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        keywords: Vec<String>,
    ) -> Result<(), BotDmErr> {
        self.moderator
            .set_keywords(user_id, group_id, keywords)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }

    async fn get_groups(&self, user_id: BotDmUserId) -> Result<Vec<Group>, BotDmErr> {
        self.moderator
            .get_groups_by_owner_id(&user_id)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
            .map(|groups| {
                groups
                    .into_iter()
                    .map(|group| Group {
                        id: group.id,
                        name: group.name,
                        notifications_enabled: group.notifications_enabled,
                        dry_mode_enabled: group.dry_mode_enabled,
                    })
                    .collect()
            })
    }

    async fn get_keywords(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
    ) -> Result<Option<Vec<String>>, BotDmErr> {
        let keywords = self
            .moderator
            .get_keywords(user_id, group_id)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })?;
        Ok(Some(keywords))
    }

    async fn set_notifications(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        enabled: bool,
    ) -> Result<(), BotDmErr> {
        self.moderator
            .set_notifications(user_id, group_id, enabled)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }

    async fn set_dry_mode(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        enabled: bool,
    ) -> Result<(), BotDmErr> {
        self.moderator
            .set_dry_mode(user_id, group_id, enabled)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }
}
