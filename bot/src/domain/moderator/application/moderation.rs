use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::moderator::message_filter::{count_effective_lines, should_moderate};
use crate::domain::moderator::ports::{
    Err, GroupCharacterActivityRepository, GroupLineActivityRepository, GroupMemberRole,
    GroupMessage, GroupMessageActivityRepository, GroupMessageHistoryRepository, GroupModerator,
    MemberRestoreRepository, ModerationAction, ModerationEngine, ModerationNotifier,
    ModerationRepository, ModerationRule, OpenAi, OpenRouter, RecentGroupMessage,
    UserCharacterActivityRepository, UserLineActivityRepository, UserMessageActivityRepository,
    UserModerationActivityRepository,
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
                .record_message(
                    &group_message.group.id,
                    &group_message.author_id,
                    group_message.timestamp,
                    ttl,
                )
                .await?;
        }

        Ok(())
    }

    async fn track_user_characters_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let max_character_rate_limit_window = rules
            .iter()
            .filter_map(|r| r.condition.max_character_rate_limit_window())
            .map(|window| window.min(60))
            .max();

        if let Some(max_window_minutes) = max_character_rate_limit_window {
            let ttl = Duration::from_secs(max_window_minutes as u64 * 60);
            self.character_activity_repository
                .record_characters(
                    &group_message.group.id,
                    &group_message.author_id,
                    group_message.timestamp,
                    // Saturating: a message longer than u32::MAX characters
                    // cannot reach the bot, and a wrapped count would read as
                    // a short message.
                    group_message.text.chars().count().min(u32::MAX as usize) as u32,
                    ttl,
                )
                .await?;
        }

        Ok(())
    }

    async fn track_user_lines_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let max_line_rate_limit_window = rules
            .iter()
            .filter_map(|r| r.condition.max_line_rate_limit_window())
            .map(|window| window.min(60))
            .max();

        if let Some(max_window_minutes) = max_line_rate_limit_window {
            // One counter serves the whole group, so the message is measured
            // once, with the widest width any of its conditions configured.
            let chars_per_line = rules
                .iter()
                .filter_map(|r| r.condition.line_rate_limit_wrap_width())
                .fold(None, |widest, width| match (widest, width) {
                    (_, 0) | (Some(0), _) => Some(0),
                    (Some(current), width) => Some(current.max(width)),
                    (None, width) => Some(width),
                })
                .unwrap_or(0);

            let ttl = Duration::from_secs(max_window_minutes as u64 * 60);
            self.line_activity_repository
                .record_lines(
                    &group_message.group.id,
                    &group_message.author_id,
                    group_message.timestamp,
                    // An attachment adds no lines of its own — a caption-less
                    // picture weighs nothing, where counting it as the one
                    // empty line it technically is would make the limit count
                    // attachments.
                    if group_message.text.is_empty() {
                        0
                    } else {
                        count_effective_lines(&group_message.text, chars_per_line)
                            .min(u32::MAX as usize) as u32
                    },
                    ttl,
                )
                .await?;
        }

        Ok(())
    }

    /// Counts the message toward the group-wide limits, each on its own
    /// counter and only when some rule asks for that limit, the way the
    /// author's three counters above are kept.
    async fn track_group_activity_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
    ) -> Result<(), Err> {
        let group_id = &group_message.group.id;
        let max_window = |pick: fn(&ModerationRule) -> Option<u32>| {
            rules
                .iter()
                .filter_map(pick)
                .map(|window| window.min(60))
                .max()
                .map(|minutes| Duration::from_secs(minutes as u64 * 60))
        };

        if let Some(ttl) = max_window(|r| r.condition.max_group_message_rate_limit_window()) {
            self.group_activity_repository
                .record_message(group_id, group_message.timestamp, ttl)
                .await?;
        }

        if let Some(ttl) = max_window(|r| r.condition.max_group_character_rate_limit_window()) {
            self.group_character_activity_repository
                .record_characters(
                    group_id,
                    group_message.timestamp,
                    group_message.text.chars().count().min(u32::MAX as usize) as u32,
                    ttl,
                )
                .await?;
        }

        if let Some(ttl) = max_window(|r| r.condition.max_group_line_rate_limit_window()) {
            let chars_per_line = rules
                .iter()
                .filter_map(|r| r.condition.group_line_rate_limit_wrap_width())
                .fold(None, |widest, width| match (widest, width) {
                    (_, 0) | (Some(0), _) => Some(0),
                    (Some(current), width) => Some(current.max(width)),
                    (None, width) => Some(width),
                })
                .unwrap_or(0);
            self.group_line_activity_repository
                .record_lines(
                    group_id,
                    group_message.timestamp,
                    // As for the author's counter: an attachment adds no lines.
                    if group_message.text.is_empty() {
                        0
                    } else {
                        count_effective_lines(&group_message.text, chars_per_line)
                            .min(u32::MAX as usize) as u32
                    },
                    ttl,
                )
                .await?;
        }

        Ok(())
    }

    /// Keeps the message among the group's latest while some
    /// `FlaggedByOpenRouterInstruction` asks for earlier messages, as many as
    /// the most demanding one asks for.
    async fn keep_in_history_if_needed(
        &self,
        group_message: &GroupMessage,
        rules: &[ModerationRule],
        deleted: bool,
    ) -> Result<(), Err> {
        let Some(keep) = rules
            .iter()
            .filter_map(|r| r.condition.max_openrouter_context_messages())
            .max()
        else {
            return Ok(());
        };
        let group_id = &group_message.group.id;
        if deleted {
            // A new message was never recorded; an edit may have been.
            if group_message.is_edit {
                self.message_history
                    .forget_message(group_id, &group_message.message_id)
                    .await?;
            }
            return Ok(());
        }
        let message = RecentGroupMessage {
            message_id: group_message.message_id,
            author_id: group_message.author_id,
            author_name: group_message.author_name.clone(),
            text: group_message.text.clone(),
            attachment: group_message.attachment,
            timestamp: group_message.timestamp,
        };
        if group_message.is_edit {
            self.message_history.record_edit(group_id, message).await
        } else {
            self.message_history
                .record_message(group_id, message, keep)
                .await
        }
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

        if !group_message.is_edit {
            self.track_user_message_if_needed(&group_message, &rules_list)
                .await?;
            self.track_user_characters_if_needed(&group_message, &rules_list)
                .await?;
            self.track_user_lines_if_needed(&group_message, &rules_list)
                .await?;
            self.track_group_activity_if_needed(&group_message, &rules_list)
                .await?;
        }

        let matched = should_moderate(
            &group_message,
            &rules_list,
            self.activity_repository.as_ref(),
            self.character_activity_repository.as_ref(),
            self.line_activity_repository.as_ref(),
            self.moderation_activity_repository.as_ref(),
            self.group_activity_repository.as_ref(),
            self.group_character_activity_repository.as_ref(),
            self.group_line_activity_repository.as_ref(),
            self.message_history.as_ref(),
            self.openai.as_ref(),
            self.openrouter.as_ref(),
        )
        .await?;

        let mut deleted = false;
        if let Some(matched) = matched {
            if !group_message.is_edit {
                self.track_moderated_message_if_needed(&group_message, &rules_list)
                    .await?;
            }
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
        let kept = self
            .keep_in_history_if_needed(&group_message, &rules_list, deleted)
            .await;
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
