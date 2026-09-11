use super::*;
use crate::domain::moderator::message_filter::ModerationCondition;
use crate::domain::moderator::ports::{
    DeleteAuthorMessages, DeleteObserverMessages, MessageId, MessengerGroup, ModerationAction,
    UserActivityRepository, UserModerationActivityRepository,
};
use crate::infrastructure::adapters::user_activity_repo_in_memory::InMemoryUserActivityRepository;
use crate::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;
use chrono::{DateTime, Utc};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum PortCall {
    SetObserver(GroupId, UserId),
    DeleteMessage(GroupId, MessageId),
    KickMember(GroupId, UserId),
    NotifyAction(UserId, GroupId, ModerationAction),
}

#[derive(Default)]
struct MockGroupModerator {
    deleted_messages: Arc<Mutex<Vec<(GroupId, MessageId)>>>,
    kicked_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
    observer_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
    call_log: Arc<Mutex<Vec<PortCall>>>,
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

    async fn set_member_observer(&self, group_id: &GroupId, user_id: &UserId) -> Result<(), Err> {
        self.observer_members
            .lock()
            .unwrap()
            .push((*group_id, *user_id));
        self.call_log
            .lock()
            .unwrap()
            .push(PortCall::SetObserver(*group_id, *user_id));
        Ok(())
    }

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, Err> {
        Ok(messenger_group_id)
    }
}

#[derive(Default)]
struct MockModerationNotifier {
    notifications: Arc<Mutex<Vec<(UserId, GroupId, ModerationAction, String, String)>>>,
    call_log: Arc<Mutex<Vec<PortCall>>>,
    fail_notification: bool,
}

#[async_trait]
impl ModerationNotifier for MockModerationNotifier {
    async fn notify_moderation_action(
        &self,
        user_id: UserId,
        group: &Group,
        action: &ModerationAction,
        message: &str,
        reason: &str,
    ) -> Result<(), Err> {
        self.notifications.lock().unwrap().push((
            user_id,
            group.id,
            *action,
            message.to_string(),
            reason.to_string(),
        ));
        self.call_log
            .lock()
            .unwrap()
            .push(PortCall::NotifyAction(user_id, group.id, *action));
        if self.fail_notification {
            return Err("notification failed".into());
        }
        Ok(())
    }
}

struct MockModerationRepository {
    group: Option<Group>,
    rules: Vec<OwnedModerationRule>,
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
struct MockActivityRecorder {
    recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, Duration)>>>,
}

#[async_trait]
impl UserActivityRepository for MockActivityRecorder {
    async fn record_user_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.recorded
            .lock()
            .unwrap()
            .push((*group_id, *user_id, timestamp, ttl));
        Ok(())
    }

    async fn count_messages_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(0)
    }
}

#[derive(Default)]
struct MockModerationActivityRecorder {
    recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, Duration)>>>,
    count_to_return: u32,
}

#[async_trait]
impl UserModerationActivityRepository for MockModerationActivityRecorder {
    async fn record_moderated_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.recorded
            .lock()
            .unwrap()
            .push((*group_id, *user_id, timestamp, ttl));
        Ok(())
    }

    async fn count_moderated_messages_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(self.count_to_return)
    }
}

#[tokio::test]
async fn test_process_group_message_moderate_message_action() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["badword".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Contains badword here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Message must be deleted
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    // Member must NOT be kicked
    assert!(kicked_members.lock().unwrap().is_empty());
    // Notification received
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].0, 100);
    assert_eq!(notifs[0].1, 10);
    assert_eq!(notifs[0].2, ModerationAction::ModerateMessage);
}

#[tokio::test]
async fn test_process_group_message_kick_author_with_triggered_message() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Member MUST be kicked
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 777)]);
    // Message MUST be deleted
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    // Notification sent with KickAuthor
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(
        notifs[0].2,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
}

#[tokio::test]
async fn test_process_group_message_kick_author_with_delete_messages_none() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::None,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Member MUST be kicked
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 777)]);
    // Message MUST NOT be deleted
    assert!(deleted_messages.lock().unwrap().is_empty());
    // Notification sent with KickAuthor
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(
        notifs[0].2,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        }
    );
}

#[tokio::test]
async fn test_process_group_message_kick_author_with_delete_messages_all_messages() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Member MUST be kicked
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 777)]);
    // Message MUST NOT be deleted separately (SimpleX kick_member handles deleting all messages)
    assert!(deleted_messages.lock().unwrap().is_empty());
    // Notification sent with KickAuthor
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(
        notifs[0].2,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        }
    );
}

#[tokio::test]
async fn test_process_group_message_dry_mode_skips_action_but_sends_notification() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: true,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // In dry mode, nothing is deleted or kicked
    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(kicked_members.lock().unwrap().is_empty());
    // But notification is still sent
    assert_eq!(notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_process_group_message_no_match_does_nothing() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["badword".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Clean friendly message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(kicked_members.lock().unwrap().is_empty());
    assert!(notifications.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_process_group_message_kick_author_covers_and_upgrades_moderate_message() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    // Rule 1: ModerateMessage on "first"
    // Rule 2: KickAuthor on "second"
    let rule1 = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["first".to_string()],
            },
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["second".to_string()],
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule1, rule2],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        text: "first and second in the same message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 2 is KickAuthor { delete_messages: TriggeredMessage }, which covers Rule 1 (ModerateMessage).
    // Therefore, Rule 2 upgrades the planned actions: the message is deleted AND the member is kicked.
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 888)]);
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(
        notifs[0].2,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_with_triggered_message_sequence() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let observer_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            observer_members: observer_members.clone(),
            call_log: call_log.clone(),
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            call_log: call_log.clone(),
            fail_notification: false,
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Sequence MUST be strictly: SetObserver -> DeleteMessage -> NotifyAction
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 777),
            PortCall::DeleteMessage(10, 42),
            PortCall::NotifyAction(
                100,
                10,
                ModerationAction::SetAuthorObserver {
                    delete_message: DeleteObserverMessages::TriggeredMessage,
                },
            ),
        ]
    );

    assert_eq!(*observer_members.lock().unwrap(), vec![(10, 777)]);
    assert!(kicked_members.lock().unwrap().is_empty());
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    assert_eq!(notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_with_delete_message_none_sequence() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let observer_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            observer_members: observer_members.clone(),
            call_log: call_log.clone(),
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            call_log: call_log.clone(),
            fail_notification: false,
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Sequence MUST be strictly: SetObserver -> NotifyAction (no DeleteMessage)
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 777),
            PortCall::NotifyAction(
                100,
                10,
                ModerationAction::SetAuthorObserver {
                    delete_message: DeleteObserverMessages::None,
                },
            ),
        ]
    );

    assert_eq!(*observer_members.lock().unwrap(), vec![(10, 777)]);
    assert!(kicked_members.lock().unwrap().is_empty());
    assert!(deleted_messages.lock().unwrap().is_empty());
    assert_eq!(notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_dry_mode_with_triggered_message() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: true,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // In dry mode: neither SetObserver nor DeleteMessage is called; ONLY NotifyAction
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![PortCall::NotifyAction(
            100,
            10,
            ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
        )]
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_dry_mode_with_none() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: true,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    assert_eq!(
        *call_log.lock().unwrap(),
        vec![PortCall::NotifyAction(
            100,
            10,
            ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
        )]
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_notifications_disabled() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: false,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Sequence: SetObserver -> DeleteMessage (NO NotifyAction)
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 777),
            PortCall::DeleteMessage(10, 42),
        ]
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_notification_failure_does_not_fail_action()
{
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["danger".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            fail_notification: true,
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    // Even if notification fails, the overall processing must succeed (best-effort)
    assert!(app.process_group_message(msg).await.is_ok());

    // Both moderation actions were performed before notification attempt
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 777),
            PortCall::DeleteMessage(10, 42),
            PortCall::NotifyAction(
                100,
                10,
                ModerationAction::SetAuthorObserver {
                    delete_message: DeleteObserverMessages::TriggeredMessage,
                },
            ),
        ]
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_rule_order_first_match_wins() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule1 = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["first".to_string()],
            },
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["second".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule1, rule2],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        text: "first and second in the same message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 1 (SetAuthorObserver) wins
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 888),
            PortCall::DeleteMessage(10, 42),
            PortCall::NotifyAction(
                100,
                10,
                ModerationAction::SetAuthorObserver {
                    delete_message: DeleteObserverMessages::TriggeredMessage,
                },
            ),
        ]
    );
}

#[tokio::test]
async fn test_process_group_message_set_author_observer_covers_and_upgrades_moderate_message() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule1 = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["first".to_string()],
            },
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["second".to_string()],
            },
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule1, rule2],
        }),
        Arc::new(MockGroupModerator {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        text: "first and second in the same message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 2 (SetAuthorObserver { TriggeredMessage }) covers Rule 1 (ModerateMessage),
    // so the action is upgraded: author is set as observer and message is deleted.
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 888),
            PortCall::DeleteMessage(10, 42),
            PortCall::NotifyAction(
                100,
                10,
                ModerationAction::SetAuthorObserver {
                    delete_message: DeleteObserverMessages::TriggeredMessage,
                },
            ),
        ]
    );
}

#[tokio::test]
async fn test_process_group_message_message_rate_limit_triggers_on_threshold() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 1,
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserActivityRepository::new());

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        activity_repo,
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let make_msg = |msg_id: i64| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id: 42,
        text: format!("Message {msg_id}"),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    // Message 1: under limit
    app.process_group_message(make_msg(1)).await.unwrap();
    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(notifications.lock().unwrap().is_empty());

    // Message 2: under limit
    app.process_group_message(make_msg(2)).await.unwrap();
    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(notifications.lock().unwrap().is_empty());

    // Message 3: hits limit (3 messages in 1 min) -> triggers moderation
    app.process_group_message(make_msg(3)).await.unwrap();
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 3)]);
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].0, 100);
    assert_eq!(notifs[0].2, ModerationAction::ModerateMessage);
    assert!(notifs[0].4.contains("author sent 3 messages in 1 min"));
}

#[tokio::test]
async fn test_process_group_message_message_rate_limit_kick_author() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 2,
                time_window_minutes: 5,
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserActivityRepository::new());

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        activity_repo,
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let make_msg = |author_id: i64, msg_id: i64| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id,
        text: "hello".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    // User A message 1
    app.process_group_message(make_msg(111, 1)).await.unwrap();
    // User B message 1
    app.process_group_message(make_msg(222, 2)).await.unwrap();

    assert!(kicked_members.lock().unwrap().is_empty());

    // User A message 2 -> reaches limit 2 -> User A is kicked
    app.process_group_message(make_msg(111, 3)).await.unwrap();
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 111)]);
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 3)]);

    // User B is still not kicked
    assert!(!kicked_members.lock().unwrap().contains(&(10, 222)));
}

#[tokio::test]
async fn test_process_group_message_message_rate_limit_dry_mode() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: true,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 1,
                time_window_minutes: 1,
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserActivityRepository::new());

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        activity_repo,
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 999,
        text: "hi".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // Dry mode: no kick, no delete
    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(kicked_members.lock().unwrap().is_empty());
    // Notification sent
    assert_eq!(notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_process_group_message_message_rate_limit_uses_message_timestamp() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 2,
                time_window_minutes: 5,
            },
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserActivityRepository::new());

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier::default()),
        activity_repo,
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let base_time = Utc::now() - chrono::Duration::hours(1);

    // Message 1 at base_time
    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 123,
        text: "first".to_string(),
        timestamp: base_time,
        author_joined_at: None,
    })
    .await
    .unwrap();

    // Message 2 at base_time + 10 minutes (outside the 5 min window from message 1)
    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 2,
        author_id: 123,
        text: "second".to_string(),
        timestamp: base_time + chrono::Duration::minutes(10),
        author_joined_at: None,
    })
    .await
    .unwrap();

    // Message 2 should NOT trigger rate limit since 10 min > 5 min
    assert!(deleted_messages.lock().unwrap().is_empty());

    // Message 3 at base_time + 12 minutes (within 5 min window from message 2)
    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 3,
        author_id: 123,
        text: "third".to_string(),
        timestamp: base_time + chrono::Duration::minutes(12),
        author_joined_at: None,
    })
    .await
    .unwrap();

    // Message 3 MUST trigger rate limit (2 messages in 5 min window: message 2 & 3)
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 3)]);
}

#[tokio::test]
async fn test_track_user_message_called_when_message_rate_limit_rule_configured() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 5,
                time_window_minutes: 10,
            },
        },
    };

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg_time = Utc::now();
    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        text: "hello".to_string(),
        timestamp: msg_time,
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    let recorded = recorder.recorded.lock().unwrap();
    assert_eq!(
        recorded.len(),
        1,
        "record_user_message must be called exactly once"
    );
    assert_eq!(recorded[0].0, 10, "group_id must match");
    assert_eq!(recorded[0].1, 42, "user_id must match");
    assert_eq!(
        recorded[0].2, msg_time,
        "timestamp must match message timestamp"
    );
    assert_eq!(
        recorded[0].3,
        Duration::from_secs(10 * 60),
        "TTL must match 10 minutes"
    );
}

#[tokio::test]
async fn test_track_user_message_uses_max_window_across_multiple_message_rate_limit_rules() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 5,
                },
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                action: ModerationAction::KickAuthor {
                    delete_messages: DeleteAuthorMessages::None,
                },
                condition: ModerationCondition::AuthorHitsMessageRateLimit {
                    message_count: 20,
                    time_window_minutes: 25,
                },
            },
        },
    ];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        text: "hello".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    let recorded = recorder.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    // TTL must be based on the maximum window among rules: max(5, 25) = 25 minutes
    assert_eq!(recorded[0].3, Duration::from_secs(25 * 60));
}

#[tokio::test]
async fn test_track_user_message_not_called_when_no_message_rate_limit_rules() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["badword".to_string()],
                },
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 40,
                },
            },
        },
    ];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        text: "clean message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    // When there are no RateLimit rules, record_user_message must NOT be called
    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_user_message must not be called when group has no rate limit rules"
    );
}

#[tokio::test]
async fn test_track_user_message_not_called_when_group_has_empty_rules() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        text: "hello".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_user_message must not be called when group has empty rules"
    );
}

#[tokio::test]
async fn test_track_user_message_not_called_when_message_rate_limit_window_is_zero() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 5,
                time_window_minutes: 0,
            },
        },
    }];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        text: "hello".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_user_message must not be called when time_window_minutes is 0"
    );
}

#[tokio::test]
async fn test_track_user_message_ttl_capped_at_60_minutes() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::AuthorHitsMessageRateLimit {
                message_count: 5,
                time_window_minutes: 120, // exceeds 60 min ceiling
            },
        },
    }];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        text: "hello".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(msg).await.unwrap();

    let recorded = recorder.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].3,
        Duration::from_secs(60 * 60),
        "TTL must be capped at 60 minutes"
    );
}

#[tokio::test]
async fn test_process_group_message_moderation_rate_limit_triggers_and_kicks() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                action: ModerationAction::KickAuthor {
                    delete_messages: DeleteAuthorMessages::TriggeredMessage,
                },
                condition: ModerationCondition::AuthorHitsModerationRateLimit {
                    message_count: 2,
                    time_window_minutes: 60,
                },
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["spam".to_string()],
                },
            },
        },
    ];

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserActivityRepository::new());
    let moderation_activity_repo = Arc::new(InMemoryUserModerationActivityRepository::new());

    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        activity_repo,
        moderation_activity_repo,
    );

    let make_msg = |msg_id: i64, text: &str| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id: 777,
        text: text.to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    // Message 1: contains spam -> moderated via rule 2 (ModerateMessage).
    // Effective moderation count including this message = 0 prior + 1 = 1 < limit 2 -> not kicked yet.
    app.process_group_message(make_msg(1, "buy spam now"))
        .await
        .unwrap();
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 1)]);
    assert!(kicked_members.lock().unwrap().is_empty());

    // Message 2: contains spam -> moderated again. Effective count = 1 prior + this one = 2 >= limit 2,
    // so rule 1 triggers KickAuthor on this same message.
    app.process_group_message(make_msg(2, "more spam here"))
        .await
        .unwrap();
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 1), (10, 2)]);
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 777)]);
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 2);
    assert_eq!(
        notifs[1].2,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage
        }
    );
    assert!(
        notifs[1]
            .4
            .contains("author had 2 messages moderated in 60 min")
    );
}

#[tokio::test]
async fn test_track_moderated_message_called_only_when_message_moderated() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                action: ModerationAction::KickAuthor {
                    delete_messages: DeleteAuthorMessages::None,
                },
                condition: ModerationCondition::AuthorHitsModerationRateLimit {
                    message_count: 5,
                    time_window_minutes: 30,
                },
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["badword".to_string()],
                },
            },
        },
    ];

    let mod_recorder = Arc::new(MockModerationActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserActivityRepository::new()),
        mod_recorder.clone(),
    );

    let clean_msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        text: "clean message".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    // Clean message: should NOT record into moderation activity recorder
    app.process_group_message(clean_msg).await.unwrap();
    assert!(
        mod_recorder.recorded.lock().unwrap().is_empty(),
        "clean message must not be recorded in moderation activity"
    );

    let bad_time = Utc::now();
    let bad_msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 2,
        author_id: 42,
        text: "contains badword here".to_string(),
        timestamp: bad_time,
        author_joined_at: None,
    };

    // Moderated message: MUST record into moderation activity recorder
    app.process_group_message(bad_msg).await.unwrap();
    let recorded = mod_recorder.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1, "moderated message must be recorded once");
    assert_eq!(recorded[0].0, 10);
    assert_eq!(recorded[0].1, 42);
    assert_eq!(recorded[0].2, bad_time);
    assert_eq!(recorded[0].3, Duration::from_secs(30 * 60));
}

#[tokio::test]
async fn test_track_moderated_message_not_called_when_no_moderation_rate_limit_rules() {
    let group = Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    };
    // Group only has keyword rules, no AuthorHitsModerationRateLimit rules
    let rules = vec![OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["badword".to_string()],
            },
        },
    }];

    let mod_recorder = Arc::new(MockModerationActivityRecorder::default());
    let app = ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserActivityRepository::new()),
        mod_recorder.clone(),
    );

    let bad_msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        text: "contains badword here".to_string(),
        timestamp: Utc::now(),
        author_joined_at: None,
    };

    app.process_group_message(bad_msg).await.unwrap();

    // Even though message was moderated, no AuthorHitsModerationRateLimit rule exists,
    // so record_moderated_message must NOT be called.
    assert!(
        mod_recorder.recorded.lock().unwrap().is_empty(),
        "record_moderated_message must not be called when group has no AuthorHitsModerationRateLimit rules"
    );
}

// ---------------------------------------------------------------------------
// Rule saving: conditions are validated before they reach the repository.
// ---------------------------------------------------------------------------

fn app_owning_group(group_id: GroupId, owner_id: UserId) -> ModeratorApplication {
    ModeratorApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: group_id,
                owner_id,
                name: "Test Group".to_string(),
                notifications_enabled: true,
                dry_mode_enabled: false,
            }),
            rules: vec![],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
    )
}

#[tokio::test]
async fn test_set_group_rules_rejects_invalid_condition() {
    let app = app_owning_group(10, 100);

    let err = app
        .set_group_rules(
            100,
            10,
            vec![ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["a".repeat(101)],
                },
            }],
        )
        .await
        .expect_err("an over-long keyword should be rejected");

    assert!(
        err.to_string().contains("Keyword too long"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_set_group_rules_accepts_valid_conditions() {
    let app = app_owning_group(10, 100);

    app.set_group_rules(
        100,
        10,
        vec![ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["badword".to_string()],
            },
        }],
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_set_group_rules_checks_ownership_before_validating() {
    let app = app_owning_group(10, 100);

    // User 999 does not own the group; the invalid keyword must not be what is
    // reported, so that rule contents are never validated for a non-owner.
    let err = app
        .set_group_rules(
            999,
            10,
            vec![ModerationRule {
                action: ModerationAction::ModerateMessage,
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["a".repeat(101)],
                },
            }],
        )
        .await
        .expect_err("a non-owner should be rejected");

    assert!(
        err.to_string().contains("is not the owner"),
        "unexpected error: {err}"
    );
}
