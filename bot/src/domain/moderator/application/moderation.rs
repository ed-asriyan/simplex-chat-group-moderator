use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::moderator::message_filter::should_moderate;
use crate::domain::moderator::ports::{
    Err, GroupMemberRole, GroupMessage, GroupModerator, MemberRestoreRepository, ModerationAction,
    ModerationEngine, ModerationNotifier, ModerationRepository, ModerationRule,
    UserActivityRepository, UserModerationActivityRepository,
};

#[cfg(test)]
mod tests;

pub struct MessageModerationApplication {
    repository: Arc<dyn ModerationRepository>,
    group_moderator: Arc<dyn GroupModerator>,
    notifier: Arc<dyn ModerationNotifier>,
    activity_repository: Arc<dyn UserActivityRepository>,
    moderation_activity_repository: Arc<dyn UserModerationActivityRepository>,
    restores: Arc<dyn MemberRestoreRepository>,
}

impl MessageModerationApplication {
    pub fn new(
        repository: Arc<dyn ModerationRepository>,
        group_moderator: Arc<dyn GroupModerator>,
        notifier: Arc<dyn ModerationNotifier>,
        activity_repository: Arc<dyn UserActivityRepository>,
        moderation_activity_repository: Arc<dyn UserModerationActivityRepository>,
        restores: Arc<dyn MemberRestoreRepository>,
    ) -> Self {
        Self {
            repository,
            group_moderator,
            notifier,
            activity_repository,
            moderation_activity_repository,
            restores,
        }
    }

    async fn track_user_message_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        // The condition can sit at any depth inside a rule's tree, so this asks
        // the tree rather than matching on the rule's root condition.
        let max_message_rate_limit_window = rules
            .iter()
            .filter_map(|r| r.condition.max_message_rate_limit_window())
            .map(|window| window.min(60))
            .max();

        if let Some(max_window_minutes) = max_message_rate_limit_window {
            let ttl = Duration::from_secs(max_window_minutes as u64 * 60);
            self.activity_repository
                .record_user_message(
                    &group_message.group.id,
                    &group_message.author_id,
                    group_message.timestamp,
                    ttl,
                )
                .await?;
        }

        Ok(())
    }

    async fn track_moderated_message_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let max_moderation_window = rules
            .iter()
            .filter_map(|r| r.condition.max_moderation_rate_limit_window())
            .map(|window| window.min(60))
            .max();

        if let Some(max_window_minutes) = max_moderation_window {
            let ttl = Duration::from_secs(max_window_minutes as u64 * 60);
            self.moderation_activity_repository
                .record_moderated_message(
                    &group_message.group.id,
                    &group_message.author_id,
                    group_message.timestamp,
                    ttl,
                )
                .await?;
        }

        Ok(())
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

        self.track_user_message_if_needed(&group_message, &rules_list)
            .await?;

        if let Some(matched) = should_moderate(
            &group_message,
            &rules_list,
            self.activity_repository.as_ref(),
            self.moderation_activity_repository.as_ref(),
        )
        .await?
        {
            self.track_moderated_message_if_needed(&group_message, &rules_list)
                .await?;
            let group = self
                .repository
                .get_group_by_messenger_id(&group_message.group.id)
                .await?;

            let dry_mode = group.as_ref().is_some_and(|g| g.dry_mode_enabled);

            if !dry_mode {
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

        self.repository
            .set_group_name(&group_message.group.id, &group_message.group.name)
            .await?;

        match bookkeeping {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
