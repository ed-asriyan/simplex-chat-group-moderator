//! `SetAuthorObserver`: keep the author from writing, for a while or for good.

#[cfg(test)]
mod tests;

use super::{Action, ActionContext, ActionReport, Effect, Silence, ActionOn};
use crate::domain::moderator::ports::{Err, GroupMemberRole};
use crate::domain::moderator::rules::common::member::Member;
use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};

/// The longest an author may be held as an observer before the bot restores
/// them: 30 days. It caps timed restrictions only — `0` means "indefinitely",
/// which is a different thing from a very long timer and stays allowed.
pub const MAX_OBSERVER_DURATION_MINUTES: u32 = 30 * 24 * 60;

/// Makes the author an observer, who can read the group but not write to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAuthorObserver {
    /// How long the author stays an observer before the bot restores them to
    /// member. `0` restricts them indefinitely, with no restore scheduled.
    #[serde(default)]
    pub duration_minutes: u32,
}

impl Action for SetAuthorObserver {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        let duration_minutes = self.duration_minutes;
        if duration_minutes > MAX_OBSERVER_DURATION_MINUTES {
            return Err(format!(
                "Observer duration too long: {duration_minutes} minutes, maximum is {MAX_OBSERVER_DURATION_MINUTES} (30 days)"
            )
            .into());
        }
        Ok(())
    }

    /// Holding the author indefinitely is the strictest restriction, and a
    /// longer timer is stricter than a shorter one.
    fn effect(&self) -> Effect {
        Effect {
            author_silenced: match self.duration_minutes {
                0 => Silence::Forever,
                minutes => Silence::For { minutes },
            },
            ..Effect::default()
        }
    }

    fn execution_rank(&self) -> u8 {
        0
    }
}

#[async_trait]
impl<S: Member + ?Sized> ActionOn<S> for SetAuthorObserver {
    async fn execute(&self, ctx: &ActionContext<'_, S>) -> Result<ActionReport, Err> {
        let member = ctx.subject;
        ctx.ports
            .group_moderator
            .set_member_role(
                &member.group().id,
                &member.member_id(),
                GroupMemberRole::Observer,
            )
            .await?;
        let scheduled = if self.duration_minutes > 0 {
            ctx.ports
                .restores
                .save(
                    &member.group().id,
                    &member.member_id(),
                    member.timestamp() + ChronoDuration::minutes(i64::from(self.duration_minutes)),
                )
                .await
        } else {
            // Indefinitely means indefinitely: a restore an earlier timed
            // restriction scheduled would otherwise still lift this one.
            ctx.ports
                .restores
                .delete_for_member(&member.group().id, &member.member_id())
                .await
        };
        Ok(ActionReport {
            bookkeeping_error: scheduled.err(),
        })
    }
}
