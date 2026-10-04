use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::bot_dm::ports::{
    Err as BotDmErr, Group, GroupId as BotDmGroupId, GroupInvitation as BotDmGroupInvitation,
    GroupOperations, JoinError, UserId as BotDmUserId,
};
use crate::domain::moderator::ports::{GroupAdministration, GroupConfig, MessengerGroup};

/// Bridges the `bot_dm` bounded context to the `moderator` bounded context by
/// implementing `bot_dm::GroupOperations` on top of `moderator::GroupAdministration`.
pub struct CrossDomainRouter {
    group_administration: Arc<dyn GroupAdministration>,
}

impl CrossDomainRouter {
    pub fn new(group_administration: Arc<dyn GroupAdministration>) -> Self {
        Self {
            group_administration,
        }
    }
}

#[async_trait]
impl GroupOperations for CrossDomainRouter {
    async fn try_join_group(
        &self,
        user_id: BotDmUserId,
        invitation: &BotDmGroupInvitation,
    ) -> Result<Group, JoinError> {
        let invited = MessengerGroup {
            id: invitation.group_id,
            name: invitation.group_name.clone(),
        };
        let group = self
            .group_administration
            .try_join_group(user_id, &invited)
            .await
            .map_err(|e| -> JoinError { e.to_string().into() })?;
        Ok(Group {
            id: group.id,
            name: group.name,
        })
    }

    async fn get_groups(&self, user_id: BotDmUserId) -> Result<Vec<Group>, BotDmErr> {
        self.group_administration
            .get_groups_by_owner_id(&user_id)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
            .map(|groups| {
                groups
                    .into_iter()
                    .map(|group| Group {
                        id: group.id,
                        name: group.name,
                    })
                    .collect()
            })
    }

    async fn set_config_json(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        json: &str,
    ) -> Result<(), BotDmErr> {
        let config: GroupConfig =
            serde_json::from_str(json).map_err(|e| -> BotDmErr { e.to_string().into() })?;
        self.group_administration
            .set_group_config(user_id, group_id, config)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }

    async fn get_config_json(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
    ) -> Result<String, BotDmErr> {
        let config = self
            .group_administration
            .get_group_config(user_id, group_id)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })?;
        serde_json::to_string(&config).map_err(|e| -> BotDmErr { e.to_string().into() })
    }
}
