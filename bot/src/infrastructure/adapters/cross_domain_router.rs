use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::bot_dm::ports::{
    Err as BotDmErr, Group, GroupId as BotDmGroupId, GroupInvitation as BotDmGroupInvitation,
    GroupOperations, JoinError, UserId as BotDmUserId,
};
use crate::domain::moderator::ports::{
    GroupAdministration, GroupInvitation as ModGroupInvitation, MessengerGroup, ModerationRule,
};

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
        let mod_invitation = ModGroupInvitation {
            group: MessengerGroup {
                id: invitation.group.id,
                name: invitation.group.name.clone(),
            },
            is_moderator: invitation.is_moderator,
        };
        let group_id = self
            .group_administration
            .try_join_group(user_id, &mod_invitation)
            .await
            .map_err(|e| -> JoinError { e.to_string().into() })?;
        Ok(Group {
            id: group_id,
            name: invitation.group.name.clone(),
            notifications_enabled: true,
            dry_mode_enabled: false,
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
                        notifications_enabled: group.notifications_enabled,
                        dry_mode_enabled: group.dry_mode_enabled,
                    })
                    .collect()
            })
    }

    async fn set_rules_json(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        json: &str,
    ) -> Result<(), BotDmErr> {
        let rules: Vec<ModerationRule> =
            serde_json::from_str(json).map_err(|e| -> BotDmErr { e.to_string().into() })?;
        self.group_administration
            .set_group_rules(user_id, group_id, rules)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }

    async fn get_rules_json(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
    ) -> Result<Option<String>, BotDmErr> {
        let rules = self
            .group_administration
            .get_group_rules(user_id, group_id)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })?;

        let rules_list: Vec<ModerationRule> = rules.into_iter().map(|o| o.rule).collect();
        let json =
            serde_json::to_string(&rules_list).map_err(|e| -> BotDmErr { e.to_string().into() })?;
        Ok(Some(json))
    }

    async fn set_notifications(
        &self,
        user_id: BotDmUserId,
        group_id: BotDmGroupId,
        enabled: bool,
    ) -> Result<(), BotDmErr> {
        self.group_administration
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
        self.group_administration
            .set_dry_mode(user_id, group_id, enabled)
            .await
            .map_err(|e| -> BotDmErr { e.to_string().into() })
    }
}
