use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use std::sync::Arc;

use crate::domain::moderator::ports::{
    Err, GroupCharacterActivityRepository, GroupLineActivityRepository, GroupMemberRole,
    GroupMessage, GroupMessageActivityRepository, GroupMessageHistoryRepository, GroupModerator,
    MemberRestoreRepository, ModerationAction, ModerationEngine, ModerationNotifier,
    ModerationRepository, ModerationRule, OpenAi, OpenRouter, UserCharacterActivityRepository,
    UserLineActivityRepository, UserMessageActivityRepository, UserModerationActivityRepository,
};
use crate::domain::moderator::rules::{
    ConditionPorts, record_activity, record_moderated, record_outcome, should_moderate,
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

        let matched = should_moderate(&group_message, &rules_list, &ports).await?;

        let mut deleted = false;
        if let Some(matched) = matched {
            record_moderated(&group_message, &rules_list, &ports).await?;
            let group = self
                .repository
                .get_group_by_messenger_id(&group_message.group.id)
                .await?;

            let dry_mode = group.as_ref().is_some_and(|g| g.dry_mode_enabled);

            if !dry_mode {
                deleted = matched.actions.iter().any(|action| match action {
                    ModerationAction::ModerateMessage => true,
                    ModerationAction::KickAuthor {
                        delete_all_messages,
                    } => *delete_all_messages,
                    ModerationAction::SetAuthorObserver { .. } => false,
                });
                for action in &matched.actions {
                    match action {
                        ModerationAction::SetAuthorObserver { duration_minutes } => {
                            self.group_moderator
                                .set_member_role(
                                    &group_message.group.id,
                                    &group_message.author_id,
                                    GroupMemberRole::Observer,
                                )
                                .await?;
                            let scheduled = if *duration_minutes > 0 {
                                self.restores
                                    .save(
                                        &group_message.group.id,
                                        &group_message.author_id,
                                        group_message.timestamp
                                            + ChronoDuration::minutes(*duration_minutes as i64),
                                    )
                                    .await
                            } else {
                                // Indefinitely means indefinitely: a restore an
                                // earlier timed restriction scheduled would
                                // otherwise still lift this one.
                                self.restores
                                    .delete_for_member(
                                        &group_message.group.id,
                                        &group_message.author_id,
                                    )
                                    .await
                            };
                            // Bookkeeping the bot keeps for itself: failing it
                            // must not call off the moderation of the message
                            // or the owner's notification. It is reported once
                            // everything else has run.
                            bookkeeping = bookkeeping.or(scheduled.err());
                        }
                        ModerationAction::ModerateMessage => {
                            self.group_moderator
                                .delete_message(&group_message.group.id, &group_message.message_id)
                                .await?;
                        }
                        ModerationAction::KickAuthor {
                            delete_all_messages,
                        } => {
                            self.group_moderator
                                .kick_member(
                                    &group_message.group.id,
                                    &group_message.author_id,
                                    *delete_all_messages,
                                )
                                .await?;
                            // Nobody to restore once they are out of the group.
                            let cancelled = self
                                .restores
                                .delete_for_member(
                                    &group_message.group.id,
                                    &group_message.author_id,
                                )
                                .await;
                            bookkeeping = bookkeeping.or(cancelled.err());
                        }
                    }
                }
            }

            if let Some(group) = group
                && group.notifications_enabled
            {
                // Best-effort: a failed notification must not undo moderation.
                let _ = self
                    .notifier
                    .notify_moderation_action(
                        group.owner_id,
                        &group,
                        &matched.actions,
                        &group_message.text,
                        &matched.reasons,
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
}
