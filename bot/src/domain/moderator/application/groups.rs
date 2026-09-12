use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::moderator::ports::{
    Err, Group, GroupAdministration, GroupId, GroupInvitation, GroupModerator, MessengerGroupId,
    ModerationRepository, ModerationRule, OwnedModerationRule, UserId,
};

#[cfg(test)]
mod tests;

pub struct GroupAdministrationApplication {
    repository: Arc<dyn ModerationRepository>,
    group_moderator: Arc<dyn GroupModerator>,
}

impl GroupAdministrationApplication {
    pub fn new(
        repository: Arc<dyn ModerationRepository>,
        group_moderator: Arc<dyn GroupModerator>,
    ) -> Self {
        Self {
            repository,
            group_moderator,
        }
    }

    async fn check_ownership(&self, user_id: UserId, group_id: GroupId) -> Result<(), Err> {
        match self.repository.get_owner_by_id(&group_id).await? {
            None => Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                Err(format!("User {} is not the owner of group {}", user_id, group_id).into())
            }
            Some(_) => Ok(()),
        }
    }
}

#[async_trait]
impl GroupAdministration for GroupAdministrationApplication {
    async fn try_join_group(
        &self,
        owner_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<GroupId, Err> {
        let messenger_group_id = invitation.group.id;
        let existing_owner = self
            .repository
            .get_owner_by_messenger_id(&messenger_group_id)
            .await?;
        if existing_owner.is_some() {
            return Err(format!("Group {} is already registered", messenger_group_id).into());
        }
        self.group_moderator.join_group(invitation.group.id).await?;
        let group_id = self
            .repository
            .save_owner(&messenger_group_id, &invitation.group.name, &owner_id)
            .await?;
        Ok(group_id)
    }

    async fn remove_group(&self, messenger_group_id: MessengerGroupId) -> Result<(), Err> {
        self.repository.delete_group_data(&messenger_group_id).await
    }

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err> {
        self.repository.get_groups_by_owner_id(owner_id).await
    }

    async fn get_group_rules(
        &self,
        user_id: UserId,
        group_id: GroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err> {
        self.check_ownership(user_id, group_id).await?;
        self.repository.get_group_rules(&group_id).await
    }

    async fn set_group_rules(
        &self,
        user_id: UserId,
        group_id: GroupId,
        rules: Vec<ModerationRule>,
    ) -> Result<(), Err> {
        self.check_ownership(user_id, group_id).await?;

        let mut rules = rules;
        for rule in &mut rules {
            rule.normalize_and_validate()?;
        }

        self.repository.set_group_rules(&group_id, &rules).await?;
        Ok(())
    }

    async fn set_notifications(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err> {
        self.check_ownership(user_id, group_id).await?;
        self.repository
            .set_notifications_enabled(&group_id, enabled)
            .await
    }

    async fn set_dry_mode(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err> {
        self.check_ownership(user_id, group_id).await?;
        self.repository
            .set_dry_mode_enabled(&group_id, enabled)
            .await?;
        // Turning dry mode on also enables notifications so the owner can
        // see what the bot *would* have moderated.
        if enabled {
            self.repository
                .set_notifications_enabled(&group_id, true)
                .await?;
        }
        Ok(())
    }
}
