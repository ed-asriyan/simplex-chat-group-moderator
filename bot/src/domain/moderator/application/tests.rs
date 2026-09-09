use super::*;
use crate::domain::moderator::message_filter::RuleCondition;
use crate::domain::moderator::ports::{MessageId, MessengerGroup};
use std::sync::Mutex;

#[derive(Default)]
struct MockGroupModerator {
    deleted_messages: Arc<Mutex<Vec<(GroupId, MessageId)>>>,
    kicked_members: Arc<Mutex<Vec<(GroupId, UserId)>>>,
}

#[async_trait]
impl GroupModerator for MockGroupModerator {
    async fn delete_message(&self, group_id: &GroupId, message_id: &MessageId) -> Result<(), Err> {
        self.deleted_messages
            .lock()
            .unwrap()
            .push((*group_id, *message_id));
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Contains badword here".to_string(),
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        text: "Contains danger here".to_string(),
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Contains danger here".to_string(),
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        text: "Clean friendly message".to_string(),
    };

    app.process_group_message(msg).await.unwrap();

    assert!(deleted_messages.lock().unwrap().is_empty());
    assert!(kicked_members.lock().unwrap().is_empty());
    assert!(notifications.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_process_group_message_rule_order_first_match_wins() {
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
            condition: RuleCondition::WordsBlacklist {
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
            condition: RuleCondition::WordsBlacklist {
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
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
        }),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        text: "first and second in the same message".to_string(),
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 1 wins -> ModerateMessage only
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    assert!(kicked_members.lock().unwrap().is_empty());
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].2, ModerationAction::ModerateMessage);
}
