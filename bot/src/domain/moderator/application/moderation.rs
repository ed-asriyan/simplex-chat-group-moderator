use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::moderator::ports::{
    Err, GroupCharacterActivityRepository, GroupLineActivityRepository, GroupMessage,
    GroupMessageActivityRepository, GroupMessageHistoryRepository, GroupModerator, MemberJoin,
    MemberRestoreRepository, ModerationEngine, ModerationNotifier, ModerationReport,
    ModerationRepository, ModerationRule, OpenAi, OpenRouter, TriggerEvent,
    UserCharacterActivityRepository, UserLineActivityRepository, UserMessageActivityRepository,
    UserModerationActivityRepository,
};
use crate::domain::moderator::rules::{
    ActionPorts, ConditionPorts, execute_join_actions, execute_message_actions, record_activity,
    record_moderated, record_outcome, should_moderate_join, should_moderate_message,
};

#[cfg(test)]
mod tests;

pub struct MessageModerationApplication {
    repository: Arc<dyn ModerationRepository>,
    group_moderator: Arc<dyn GroupModerator>,
    notifier: Arc<dyn ModerationNotifier>,
    activity_repository: Arc<dyn UserMessageActivityRepository>,
    character_activity_repository: Arc<dyn UserCharacterActivityRepository>,
    line_activity_repository: Arc<dyn UserLineActivityRepository>,
    moderation_activity_repository: Arc<dyn UserModerationActivityRepository>,
    group_activity_repository: Arc<dyn GroupMessageActivityRepository>,
    group_character_activity_repository: Arc<dyn GroupCharacterActivityRepository>,
    group_line_activity_repository: Arc<dyn GroupLineActivityRepository>,
    message_history: Arc<dyn GroupMessageHistoryRepository>,
    restores: Arc<dyn MemberRestoreRepository>,
    openai: Arc<dyn OpenAi>,
    openrouter: Arc<dyn OpenRouter>,
}

impl MessageModerationApplication {
    pub fn new(
        repository: Arc<dyn ModerationRepository>,
        group_moderator: Arc<dyn GroupModerator>,
        notifier: Arc<dyn ModerationNotifier>,
        activity_repository: Arc<dyn UserMessageActivityRepository>,
        character_activity_repository: Arc<dyn UserCharacterActivityRepository>,
        line_activity_repository: Arc<dyn UserLineActivityRepository>,
        moderation_activity_repository: Arc<dyn UserModerationActivityRepository>,
        group_activity_repository: Arc<dyn GroupMessageActivityRepository>,
        group_character_activity_repository: Arc<dyn GroupCharacterActivityRepository>,
        group_line_activity_repository: Arc<dyn GroupLineActivityRepository>,
        message_history: Arc<dyn GroupMessageHistoryRepository>,
        restores: Arc<dyn MemberRestoreRepository>,
        openai: Arc<dyn OpenAi>,
        openrouter: Arc<dyn OpenRouter>,
    ) -> Self {
        Self {
            repository,
            group_moderator,
            notifier,
            activity_repository,
            character_activity_repository,
            line_activity_repository,
            moderation_activity_repository,
            group_activity_repository,
            group_character_activity_repository,
            group_line_activity_repository,
            message_history,
            restores,
            openai,
            openrouter,
        }
    }

    /// Every port the rules may read or record into, as one bundle.
    fn ports(&self) -> ConditionPorts<'_> {
        ConditionPorts {
            activity_repo: self.activity_repository.as_ref(),
            character_activity_repo: self.character_activity_repository.as_ref(),
            line_activity_repo: self.line_activity_repository.as_ref(),
            moderation_activity_repo: self.moderation_activity_repository.as_ref(),
            group_activity_repo: self.group_activity_repository.as_ref(),
            group_character_activity_repo: self.group_character_activity_repository.as_ref(),
            group_line_activity_repo: self.group_line_activity_repository.as_ref(),
            message_history: self.message_history.as_ref(),
            openai: self.openai.as_ref(),
            openrouter: self.openrouter.as_ref(),
        }
    }

    /// Every port an action may act through, as one bundle.
    fn action_ports(&self) -> ActionPorts<'_> {
        ActionPorts {
            group_moderator: self.group_moderator.as_ref(),
            restores: self.restores.as_ref(),
        }
    }
}

#[async_trait]
impl ModerationEngine for MessageModerationApplication {
    async fn process_group_message(&self, group_message: GroupMessage) -> Result<(), Err> {
        let mut bookkeeping: Option<Err> = None;
        let rules = self
            .repository
            .get_group_rules_by_messenger_id(&group_message.group.id)
            .await?;

        let rules_list: Vec<ModerationRule> = rules.into_iter().map(|o| o.rule).collect();

        let ports = self.ports();
        record_activity(&group_message, &rules_list, &ports).await?;

        let matched = should_moderate_message(&group_message, &rules_list, &ports).await?;

        let mut deleted = false;
        if let Some(matched) = matched {
            record_moderated(&group_message, &rules_list, &ports).await?;
            let group = self
                .repository
                .get_group_by_messenger_id(&group_message.group.id)
                .await?;

            // Stage 5 stub: every matched rule's actions are carried out and told,
            // as if all had both switches on; `/implement` applies each rule's own
            // switches (US-D1–US-D4).
            {
                let outcome =
                    execute_message_actions(&group_message, &matched.actions, &self.action_ports())
                        .await?;
                deleted = outcome.message_deleted;
                // Bookkeeping the bot keeps for itself: failing it must not
                // call off the moderation of the message or the owner's
                // notification. It is reported once everything else has run.
                bookkeeping = outcome.bookkeeping_error;
            }

            if let Some(group) = group {
                // Best-effort: a failed notification must not undo moderation.
                let _ = self
                    .notifier
                    .notify_moderation(
                        group.owner_id,
                        &group,
                        &ModerationReport {
                            event: TriggerEvent::MessagePosted {
                                message: group_message.text.clone(),
                            },
                            member_name: group_message.author_name.clone(),
                            performed: matched.actions_as_written(),
                            would_perform: Vec::new(),
                            reasons: matched.reasons.clone(),
                        },
                    )
                    .await;
            }
        }

        // Kept for the next messages' context only now that this one has been
        // moderated: it is no context for itself, and one the bot deleted is
        // gone from the chat the members see.
        let kept = record_outcome(&group_message, &rules_list, deleted, &ports).await;
        bookkeeping = bookkeeping.or(kept.err());

        self.repository
            .set_group_name(&group_message.group.id, &group_message.group.name)
            .await?;

        match bookkeeping {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn process_member_join(&self, join: MemberJoin) -> Result<(), Err> {
        let Some(group) = self
            .repository
            .get_group_by_messenger_id(&join.group.id)
            .await?
        else {
            return Ok(());
        };
        let rules: Vec<ModerationRule> = self
            .repository
            .get_group_rules_by_messenger_id(&join.group.id)
            .await?
            .into_iter()
            .map(|owned| owned.rule)
            .collect();

        let mut bookkeeping: Option<Err> = None;
        if let Some(matched) = should_moderate_join(&join, &rules, &self.ports()).await? {
            // Stage 5 stub: see `process_group_message`.
            {
                let outcome =
                    execute_join_actions(&join, &matched.actions, &self.action_ports()).await?;
                // As for a message: the bot's own records failing must not call
                // off the owner's notification.
                bookkeeping = outcome.bookkeeping_error;
            }
            {
                // Best-effort: a failed notification must not undo moderation.
                let _ = self
                    .notifier
                    .notify_moderation(
                        group.owner_id,
                        &group,
                        &ModerationReport {
                            event: TriggerEvent::MemberJoined,
                            member_name: join.display_name.clone(),
                            performed: matched.actions_as_written(),
                            would_perform: Vec::new(),
                            reasons: matched.reasons.clone(),
                        },
                    )
                    .await;
            }
        }

        self.repository
            .set_group_name(&join.group.id, &join.group.name)
            .await?;

        match bookkeeping {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
