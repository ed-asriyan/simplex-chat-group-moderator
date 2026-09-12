//! Port fakes shared by the use-case test modules under this one.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

use crate::domain::moderator::ports::{
    Err, Group, GroupId, GroupMemberRole, GroupModerator, MemberRestoreRepository, MessageId,
    MessengerGroupId, ModerationAction, ModerationRepository, ModerationRule, OwnedModerationRule,
    ScheduledMemberRestore, UserId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortCall {
    SetObserver(GroupId, UserId),
    DeleteMessage(GroupId, MessageId),
    KickMember(GroupId, UserId),
    NotifyAction(UserId, GroupId, Vec<ModerationAction>),
}

#[derive(Default)]
pub struct MockGroupModerator {
    pub deleted_messages: Arc<Mutex<Vec<(GroupId, MessageId)>>>,
    pub kicked_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
    pub observer_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
    pub restored_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
    pub call_log: Arc<Mutex<Vec<PortCall>>>,
    pub fail_role_change: bool,
}

#[async_trait]
impl GroupModerator for MockGroupModerator {
    async fn delete_message(&self, group_id: &GroupId, message_id: &MessageId) -> Result<(), Err> {
        self.deleted_messages
            .lock()
            .unwrap()
            .push((*group_id, *message_id));
        self.call_log
            .lock()
            .unwrap()
            .push(PortCall::DeleteMessage(*group_id, *message_id));
        Ok(())
    }

    async fn kick_member(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        _delete_all_messages: bool,
    ) -> Result<(), Err> {
        self.kicked_members
            .lock()
            .unwrap()
            .push((*group_id, *user_id));
        self.call_log
            .lock()
            .unwrap()
            .push(PortCall::KickMember(*group_id, *user_id));
        Ok(())
    }

    async fn set_member_role(
        &self,
        group_id: &GroupId,
        user_id: &UserId,
        role: GroupMemberRole,
    ) -> Result<(), Err> {
        if self.fail_role_change {
            return Err("role change failed".into());
        }
        match role {
            GroupMemberRole::Observer => {
                self.observer_members
                    .lock()
                    .unwrap()
                    .push((*group_id, *user_id));
                self.call_log
                    .lock()
                    .unwrap()
                    .push(PortCall::SetObserver(*group_id, *user_id));
            }
            GroupMemberRole::Member => {
                self.restored_members
                    .lock()
                    .unwrap()
                    .push((*group_id, *user_id));
            }
        }
        Ok(())
    }

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, Err> {
        Ok(messenger_group_id)
    }
}

pub struct MockModerationRepository {
    pub group: Option<Group>,
    pub rules: Vec<OwnedModerationRule>,
}

#[async_trait]
impl ModerationRepository for MockModerationRepository {
    async fn save_owner(
        &self,
        _m_gid: &MessengerGroupId,
        _name: &str,
        _owner_id: &UserId,
    ) -> Result<GroupId, Err> {
        Ok(1)
    }
    async fn get_owner_by_messenger_id(
        &self,
        _m_gid: &MessengerGroupId,
    ) -> Result<Option<UserId>, Err> {
        Ok(self.group.as_ref().map(|g| g.owner_id))
    }
    async fn get_groups_by_owner_id(&self, _owner_id: &UserId) -> Result<Vec<Group>, Err> {
        Ok(self.group.clone().into_iter().collect())
    }
    async fn get_owner_by_id(&self, _group_id: &GroupId) -> Result<Option<UserId>, Err> {
        Ok(self.group.as_ref().map(|g| g.owner_id))
    }
    async fn set_group_name(&self, _m_gid: &MessengerGroupId, _name: &str) -> Result<(), Err> {
        Ok(())
    }
    async fn get_group_rules(&self, _group_id: &GroupId) -> Result<Vec<OwnedModerationRule>, Err> {
        Ok(self.rules.clone())
    }
    async fn get_group_rules_by_messenger_id(
        &self,
        _m_gid: &MessengerGroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err> {
        Ok(self.rules.clone())
    }
    async fn set_group_rules(
        &self,
        _group_id: &GroupId,
        _rules: &[ModerationRule],
    ) -> Result<(), Err> {
        Ok(())
    }
    async fn delete_group_data(&self, _m_gid: &MessengerGroupId) -> Result<(), Err> {
        Ok(())
    }
    async fn get_group_by_messenger_id(
        &self,
        _m_gid: &MessengerGroupId,
    ) -> Result<Option<Group>, Err> {
        Ok(self.group.clone())
    }
    async fn set_notifications_enabled(
        &self,
        _group_id: &GroupId,
        _enabled: bool,
    ) -> Result<(), Err> {
        Ok(())
    }
    async fn set_dry_mode_enabled(&self, _group_id: &GroupId, _enabled: bool) -> Result<(), Err> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MockMemberRestoreRepository {
    pub saved: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>)>>>,
    pub cancelled: Arc<Mutex<Vec<(MessengerGroupId, UserId)>>>,
    pub due: Arc<Mutex<Vec<ScheduledMemberRestore>>>,
    pub deleted: Arc<Mutex<Vec<i64>>>,
    pub fail_writes: bool,
}

#[async_trait]
impl MemberRestoreRepository for MockMemberRestoreRepository {
    async fn save(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
        execute_at: DateTime<Utc>,
    ) -> Result<(), Err> {
        self.saved
            .lock()
            .unwrap()
            .push((*messenger_group_id, *member_id, execute_at));
        self.write_result()
    }

    async fn delete_for_member(
        &self,
        messenger_group_id: &MessengerGroupId,
        member_id: &UserId,
    ) -> Result<(), Err> {
        self.cancelled
            .lock()
            .unwrap()
            .push((*messenger_group_id, *member_id));
        self.write_result()
    }

    async fn list_due(&self, now: DateTime<Utc>) -> Result<Vec<ScheduledMemberRestore>, Err> {
        Ok(self
            .due
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.execute_at <= now)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: i64, execute_at: DateTime<Utc>) -> Result<(), Err> {
        self.deleted.lock().unwrap().push(id);
        self.due
            .lock()
            .unwrap()
            .retain(|r| r.id != id || r.execute_at != execute_at);
        self.write_result()
    }
}

impl MockMemberRestoreRepository {
    fn write_result(&self) -> Result<(), Err> {
        if self.fail_writes {
            return Err("restore bookkeeping failed".into());
        }
        Ok(())
    }
}
