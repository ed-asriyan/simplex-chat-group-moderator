use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use std::sync::Arc;

use crate::domain::moderator::ports::{
    Err, GroupMemberRole, GroupModerator, MemberRestoreRepository, MemberRestoreRunner,
};

#[cfg(test)]
mod tests;

/// How long a restore may keep failing before the bot gives up on it. Long
/// enough that an outage of the messenger does not strand held members.
const ABANDON_AFTER_HOURS: i64 = 24;

pub struct MemberRestoreApplication {
    restores: Arc<dyn MemberRestoreRepository>,
    group_moderator: Arc<dyn GroupModerator>,
}

impl MemberRestoreApplication {
    pub fn new(
        restores: Arc<dyn MemberRestoreRepository>,
        group_moderator: Arc<dyn GroupModerator>,
    ) -> Self {
        Self {
            restores,
            group_moderator,
        }
    }
}

#[async_trait]
impl MemberRestoreRunner for MemberRestoreApplication {
    async fn run_due_restores(&self, now: DateTime<Utc>) -> Result<(), Err> {
        let mut failure: Option<Err> = None;

        for restore in self.restores.list_due(now).await? {
            let restored = self
                .group_moderator
                .set_member_role(
                    &restore.messenger_group_id,
                    &restore.member_id,
                    GroupMemberRole::Member,
                )
                .await;

            // A failed restore keeps its row so the next run tries again —
            // unless it has been failing for a whole day, which is what a member
            // the bot can never restore looks like (they left the group, say).
            // Retrying that one every minute for the life of the process helps
            // nobody, so the obligation is dropped.
            let give_up = now - restore.execute_at > TimeDelta::hours(ABANDON_AFTER_HOURS);
            if let Err(e) = restored {
                failure = failure.or(Some(e));
                if !give_up {
                    continue;
                }
            }

            // Conditional on the deadline this run acted on: `save` reuses the
            // member's row, so an unconditional delete could drop a restriction
            // imposed while this loop was running.
            if let Err(e) = self.restores.delete(restore.id, restore.execute_at).await {
                // One member the bot cannot reach must not hold up the others.
                failure = failure.or(Some(e));
            }
        }

        match failure {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
