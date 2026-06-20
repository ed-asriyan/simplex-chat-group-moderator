use async_trait::async_trait;
use std::sync::Arc;

use super::message_filter::should_moderate;
use super::ports::{
    Err, Group, GroupId, GroupInvitation, GroupMessage, GroupModerator, MessengerGroupId,
    ModerationEngine, ModerationNotifier, ModerationRepository, ModerationRule,
    OwnedModerationRule, UserId,
};

pub struct ModeratorApplication {
    repository: Arc<dyn ModerationRepository>,
    group_moderator: Arc<dyn GroupModerator>,
    notifier: Arc<dyn ModerationNotifier>,
}

impl ModeratorApplication {
    pub fn new(
        repository: Arc<dyn ModerationRepository>,
        group_moderator: Arc<dyn GroupModerator>,
        notifier: Arc<dyn ModerationNotifier>,
    ) -> Self {
        Self {
            repository,
            group_moderator,
            notifier,
        }
    }
}

#[async_trait]
impl ModerationEngine for ModeratorApplication {
    async fn process_group_message(&self, group_message: GroupMessage) -> Result<(), Err> {
        let rules = self
            .repository
            .get_group_rules_by_messenger_id(&group_message.group.id)
            .await?;

        let rules_list: Vec<ModerationRule> = rules.into_iter().map(|o| o.rule).collect();

        if let Some(phrase) = should_moderate(&group_message.text, &rules_list) {
            let group = self
                .repository
                .get_group_by_messenger_id(&group_message.group.id)
                .await?;

            let dry_mode = group.as_ref().is_some_and(|g| g.dry_mode_enabled);

            if !dry_mode {
                self.group_moderator
                    .delete_message(&group_message.group.id, &group_message.message_id)
                    .await?;
            }

            if let Some(group) = group
                && group.notifications_enabled
            {
                // Best-effort: a failed notification must not undo moderation.
                let _ = self
                    .notifier
                    .notify_moderated_message(group.owner_id, &group, &group_message.text, &phrase)
                    .await;
            }
        }

        self.repository
            .set_group_name(&group_message.group.id, &group_message.group.name)
            .await?;

        Ok(())
    }

    async fn get_group_rules(
        &self,
        user_id: UserId,
        group_id: GroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err> {
        let owner = self.repository.get_owner_by_id(&group_id).await?;
        match owner {
            None => Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                Err(format!("User {} is not the owner of group {}", user_id, group_id).into())
            }
            Some(_) => {
                let rules = self.repository.get_group_rules(&group_id).await?;
                Ok(rules)
            }
        }
    }

    async fn set_group_rules(
        &self,
        user_id: UserId,
        group_id: GroupId,
        rules: Vec<ModerationRule>,
    ) -> Result<(), Err> {
        let owner = self.repository.get_owner_by_id(&group_id).await?;
        match owner {
            None => return Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                return Err(
                    format!("User {} is not the owner of group {}", user_id, group_id).into(),
                );
            }
            Some(_) => {}
        }

        self.repository.set_group_rules(&group_id, &rules).await?;
        Ok(())
    }

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

    async fn set_notifications(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err> {
        let owner = self.repository.get_owner_by_id(&group_id).await?;
        match owner {
            None => Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                Err(format!("User {} is not the owner of group {}", user_id, group_id).into())
            }
            Some(_) => {
                self.repository
                    .set_notifications_enabled(&group_id, enabled)
                    .await?;
                Ok(())
            }
        }
    }

    async fn set_dry_mode(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err> {
        let owner = self.repository.get_owner_by_id(&group_id).await?;
        match owner {
            None => Err(format!("Group {} is not registered", group_id).into()),
            Some(owner_id) if owner_id != user_id => {
                Err(format!("User {} is not the owner of group {}", user_id, group_id).into())
            }
            Some(_) => {
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
    }
}
