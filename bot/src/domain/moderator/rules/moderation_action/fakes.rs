//! A recording stand-in for the ports an action acts through, for the actions'
//! own tests.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use std::sync::Mutex;

use super::{ActionContext, ActionPorts, ActionReport};
use crate::domain::moderator::ports::{
    Err, GroupId, GroupMemberRole, GroupMessage, GroupModerator, MemberRestoreRepository,
    MessageId, MessengerGroup, MessengerGroupId, ScheduledMemberRestore, UserId,
};

/// The message every action test acts on: message 7 by member 42 in group 1.
pub fn message() -> GroupMessage {
    GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "group".into(),
        },
        message_id: 7,
        author_id: 42,
        author_name: "author".into(),
        text: "text".into(),
        attachment: None,
        timestamp: at(0),
        author_joined_at: None,
        is_edit: false,
    }
}

/// `minutes` past the moment [`message`] was sent.
pub fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
}

/// Writes down every call, in order, as one line each.
#[derive(Default)]
pub struct Recorder {
    pub calls: Mutex<Vec<String>>,
    pub fail_restores: bool,
}

impl Recorder {
    pub fn failing_restores() -> Self {
        Self {
            fail_restores: true,
            ..Self::default()
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    fn restore_result(&self) -> Result<(), Err> {
        if self.fail_restores {
            return Err("restore bookkeeping failed".into());
        }
        Ok(())
    }

    /// Carries out `action` on [`message`] with this recorder as every port.
    pub async fn execute(&self, action: &dyn super::Action) -> Result<ActionReport, Err> {
        let message = message();
        let ports = ActionPorts {
            group_moderator: self,
            restores: self,
        };
        let ctx = ActionContext {
            group_message: &message,
            ports: &ports,
        };
        action.execute(&ctx).await
    }
}

#[async_trait]
impl GroupModerator for Recorder {
    async fn delete_message(&self, group_id: &GroupId, message_id: &MessageId) -> Result<(), Err> {
        self.record(format!("delete message {message_id} in {group_id}"));
        Ok(())
    }

    async fn kick_member(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        delete_all_messages: bool,
    ) -> Result<(), Err> {
        self.record(format!(
            "kick {user_id} from {group_id}, delete all: {delete_all_messages}"
        ));
        Ok(())
    }

    async fn set_member_role(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        role: GroupMemberRole,
    ) -> Result<(), Err> {
        self.record(format!("make {user_id} {role:?} in {group_id}"));
        Ok(())
    }

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, Err> {
        Ok(messenger_group_id)
    }
}

#[async_trait]
impl MemberRestoreRepository for Recorder {
    async fn save(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
        execute_at: DateTime<Utc>,
    ) -> Result<(), Err> {
        self.record(format!(
            "restore {member_id} in {messenger_group_id} at {execute_at}"
        ));
        self.restore_result()
    }

    async fn delete_for_member(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
    ) -> Result<(), Err> {
        self.record(format!(
            "cancel the restore of {member_id} in {messenger_group_id}"
        ));
        self.restore_result()
    }

    async fn list_due(&self, _now: DateTime<Utc>) -> Result<Vec<ScheduledMemberRestore>, Err> {
        Ok(Vec::new())
    }

    async fn delete(&self, _id: i64, _execute_at: DateTime<Utc>) -> Result<(), Err> {
        Ok(())
    }
}
