use super::MessageModerationApplication;
use crate::domain::moderator::application::tests::{
    MockGroupModerator, MockMemberRestoreRepository, MockModerationRepository, PortCall,
};
use crate::domain::moderator::ports::actions::{KickAuthor, ModerateMessage, SetAuthorObserver};
use crate::domain::moderator::ports::conditions::{
    AuthorHitsCharacterRateLimit, AuthorHitsLineRateLimit, AuthorHitsMessageRateLimit,
    AuthorHitsModerationRateLimit, ContainsWords, ExceedsMaxLines, FlaggedByOmniModeration,
    FlaggedByOpenRouterInstruction, GroupHitsCharacterRateLimit, GroupHitsLineRateLimit,
    GroupHitsMessageRateLimit,
};
use crate::domain::moderator::ports::{
    CategoryTrigger, Err, Group, GroupId, GroupMessage, GroupMessageActivityRepository, KeyCheck,
    MessageAttachment, MessageId, MessengerGroup, MessengerGroupId, ModerationAction,
    ModerationEngine, ModerationNotifier, ModerationRule, OpenAi, OpenAiCategory,
    OpenAiCategoryTriggers, OpenAiModerationResult, OpenRouter, OpenRouterInstructionVerdict,
    OwnedModerationRule, UserCharacterActivityRepository, UserId, UserLineActivityRepository,
    UserMessageActivityRepository, UserModerationActivityRepository,
};
use crate::domain::moderator::rules::ModerationCondition;
use crate::infrastructure::adapters::group_character_activity_repo_in_memory::InMemoryGroupCharacterActivityRepository;
use crate::infrastructure::adapters::group_line_activity_repo_in_memory::InMemoryGroupLineActivityRepository;
use crate::infrastructure::adapters::group_message_activity_repo_in_memory::InMemoryGroupMessageActivityRepository;
use crate::infrastructure::adapters::group_message_history_repo_in_memory::InMemoryGroupMessageHistoryRepository;
use crate::infrastructure::adapters::user_character_activity_repo_in_memory::InMemoryUserCharacterActivityRepository;
use crate::infrastructure::adapters::user_line_activity_repo_in_memory::InMemoryUserLineActivityRepository;
use crate::infrastructure::adapters::user_message_activity_repo_in_memory::InMemoryUserMessageActivityRepository;
use crate::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct MockModerationNotifier {
    pub notifications: Arc<Mutex<Vec<(UserId, GroupId, Vec<ModerationAction>, String, String)>>>,
    pub call_log: Arc<Mutex<Vec<PortCall>>>,
    pub fail_notification: bool,
}

#[async_trait]
impl ModerationNotifier for MockModerationNotifier {
    async fn notify_moderation_action(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        message: &str,
        reasons: &[String],
    ) -> Result<(), Err> {
        self.notifications.lock().unwrap().push((
            user_id,
            group.id,
            actions.to_vec(),
            message.to_string(),
            reasons.join(", "),
        ));
        self.call_log.lock().unwrap().push(PortCall::NotifyAction(
            user_id,
            group.id,
            actions.to_vec(),
        ));
        if self.fail_notification {
            return Err("notification failed".into());
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MockActivityRecorder {
    pub recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, Duration)>>>,
}

#[async_trait]
impl UserMessageActivityRepository for MockActivityRecorder {
    async fn record_message(
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

/// Records what the character log was told, so the tests can check both that
/// it was told and what length it was given.
#[derive(Default)]
pub struct MockCharacterActivityRecorder {
    /// group, user, timestamp, character count, ttl.
    pub recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, u32, Duration)>>>,
}

#[async_trait]
impl UserCharacterActivityRepository for MockCharacterActivityRecorder {
    async fn record_characters(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        character_count: u32,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.recorded
            .lock()
            .unwrap()
            .push((*group_id, *user_id, timestamp, character_count, ttl));
        Ok(())
    }

    async fn sum_characters_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(0)
    }
}

/// Records what the line log was told, so the tests can check both that it was
/// told and how many lines it was given.
#[derive(Default)]
pub struct MockLineActivityRecorder {
    /// group, user, timestamp, line count, ttl.
    pub recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, u32, Duration)>>>,
}

#[async_trait]
impl UserLineActivityRepository for MockLineActivityRecorder {
    async fn record_lines(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        line_count: u32,
        ttl: Duration,
    ) -> Result<(), Err> {
        self.recorded
            .lock()
            .unwrap()
            .push((*group_id, *user_id, timestamp, line_count, ttl));
        Ok(())
    }

    async fn sum_lines_since(
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
pub struct MockModerationActivityRecorder {
    pub recorded: Arc<Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, Duration)>>>,
    pub count_to_return: u32,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        author_name: String::new(),
        text: "Contains badword here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
    assert_eq!(
        notifs[0].2,
        vec![ModerationAction::ModerateMessage(ModerateMessage {}),]
    );
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
            actions: vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
        vec![
            ModerationAction::ModerateMessage(ModerateMessage {}),
            ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: false,
            }),
        ]
    );
}

#[tokio::test]
async fn test_process_group_message_kick_author_without_deleting_messages() {
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
            actions: vec![ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: false,
            })],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
        vec![ModerationAction::KickAuthor(KickAuthor {
            delete_all_messages: false,
        }),]
    );
}

#[tokio::test]
async fn test_process_group_message_kick_author_deleting_all_messages() {
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
            actions: vec![ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: true,
            })],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
        vec![ModerationAction::KickAuthor(KickAuthor {
            delete_all_messages: true,
        }),]
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
            actions: vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 555,
        author_name: String::new(),
        text: "Clean friendly message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["first".to_string()],
            }),
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["second".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        author_name: String::new(),
        text: "first and second in the same message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 2 moderates the message and kicks the author, which covers Rule 1
    // (ModerateMessage alone). Both planned actions run: the message is deleted
    // AND the member is kicked.
    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 42)]);
    assert_eq!(*kicked_members.lock().unwrap(), vec![(10, 888)]);
    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(
        notifs[0].2,
        vec![
            ModerationAction::ModerateMessage(ModerateMessage {}),
            ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: false,
            }),
        ]
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
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let observer_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            observer_members: observer_members.clone(),
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            call_log: call_log.clone(),
            fail_notification: false,
        }),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
                vec![
                    ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes: 0
                    }),
                    ModerationAction::ModerateMessage(ModerateMessage {}),
                ],
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
            actions: vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0,
            })],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let observer_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));

    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            kicked_members: kicked_members.clone(),
            observer_members: observer_members.clone(),
            call_log: call_log.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            call_log: call_log.clone(),
            fail_notification: false,
        }),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
                vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0
                }),],
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
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    // In dry mode: neither SetObserver nor DeleteMessage is called; ONLY NotifyAction
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![PortCall::NotifyAction(
            100,
            10,
            vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
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
            actions: vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0,
            })],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    assert_eq!(
        *call_log.lock().unwrap(),
        vec![PortCall::NotifyAction(
            100,
            10,
            vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 0
            }),],
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
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["danger".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
                vec![
                    ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes: 0
                    }),
                    ModerationAction::ModerateMessage(ModerateMessage {}),
                ],
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
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["first".to_string()],
            }),
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["second".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        author_name: String::new(),
        text: "first and second in the same message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
                vec![
                    ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes: 0
                    }),
                    ModerationAction::ModerateMessage(ModerateMessage {}),
                ],
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["first".to_string()],
            }),
        },
    };
    let rule2 = OwnedModerationRule {
        id: 2,
        rule: ModerationRule {
            actions: vec![
                ModerationAction::SetAuthorObserver(SetAuthorObserver {
                    duration_minutes: 0,
                }),
                ModerationAction::ModerateMessage(ModerateMessage {}),
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["second".to_string()],
            }),
        },
    };

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 888,
        author_name: String::new(),
        text: "first and second in the same message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    // Rule 2 sets the author as observer and moderates the message, which covers
    // Rule 1 (ModerateMessage alone): the author is set as observer and the
    // message is deleted.
    assert_eq!(
        *call_log.lock().unwrap(),
        vec![
            PortCall::SetObserver(10, 888),
            PortCall::DeleteMessage(10, 42),
            PortCall::NotifyAction(
                100,
                10,
                vec![
                    ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes: 0
                    }),
                    ModerationAction::ModerateMessage(ModerateMessage {}),
                ],
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 3,
                    time_window_minutes: 1,
                },
            ),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserMessageActivityRepository::new());

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let make_msg = |msg_id: i64| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id: 42,
        author_name: String::new(),
        text: format!("Message {msg_id}"),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
    assert_eq!(
        notifs[0].2,
        vec![ModerationAction::ModerateMessage(ModerateMessage {}),]
    );
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
            actions: vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 2,
                    time_window_minutes: 5,
                },
            ),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserMessageActivityRepository::new());

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let make_msg = |author_id: i64, msg_id: i64| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![
                ModerationAction::ModerateMessage(ModerateMessage {}),
                ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                }),
            ],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 1,
                    time_window_minutes: 1,
                },
            ),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserMessageActivityRepository::new());

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 999,
        author_name: String::new(),
        text: "hi".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 2,
                    time_window_minutes: 5,
                },
            ),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserMessageActivityRepository::new());

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
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
        author_name: String::new(),
        text: "first".to_string(),
        attachment: None,
        timestamp: base_time,
        author_joined_at: None,
        is_edit: false,
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
        author_name: String::new(),
        text: "second".to_string(),
        attachment: None,
        timestamp: base_time + chrono::Duration::minutes(10),
        author_joined_at: None,
        is_edit: false,
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
        author_name: String::new(),
        text: "third".to_string(),
        attachment: None,
        timestamp: base_time + chrono::Duration::minutes(12),
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 10,
                },
            ),
        },
    };

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg_time = Utc::now();
    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: msg_time,
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    let recorded = recorder.recorded.lock().unwrap();
    assert_eq!(
        recorded.len(),
        1,
        "record_message must be called exactly once"
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
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsMessageRateLimit(
                    AuthorHitsMessageRateLimit {
                        message_count: 5,
                        time_window_minutes: 5,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                })],
                condition: ModerationCondition::AuthorHitsMessageRateLimit(
                    AuthorHitsMessageRateLimit {
                        message_count: 20,
                        time_window_minutes: 25,
                    },
                ),
            },
        },
    ];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    let recorded = recorder.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    // TTL must be based on the maximum window among rules: max(5, 25) = 25 minutes
    assert_eq!(recorded[0].3, Duration::from_secs(25 * 60));
}

/// The character log is written only when a rule asks about characters, and
/// what it is told is the length of the message being processed.
#[tokio::test]
async fn test_track_characters_called_when_character_rate_limit_rule_configured() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsCharacterRateLimit(
                AuthorHitsCharacterRateLimit {
                    character_count: 2000,
                    time_window_minutes: 10,
                },
            ),
        },
    };

    let messages = Arc::new(MockActivityRecorder::default());
    let characters = Arc::new(MockCharacterActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        messages.clone(),
        characters.clone(),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg_time = Utc::now();
    // Cyrillic on purpose: 6 characters, 12 UTF-8 bytes. The limit is on what
    // the author wrote, not on how it happens to be encoded.
    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "Привет".to_string(),
        attachment: None,
        timestamp: msg_time,
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    let recorded = characters.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1, "the character log must be written once");
    assert_eq!(recorded[0].0, 10, "group_id must match");
    assert_eq!(recorded[0].1, 42, "user_id must match");
    assert_eq!(recorded[0].2, msg_time, "the message timestamp is the now");
    assert_eq!(recorded[0].3, 6, "six characters, not twelve bytes");
    assert_eq!(
        recorded[0].4,
        Duration::from_secs(10 * 60),
        "ttl is the window"
    );

    assert!(
        messages.recorded.lock().unwrap().is_empty(),
        "a character rule must not start message tracking"
    );
}

/// A group with only a message rate limit pays nothing for characters.
#[tokio::test]
async fn test_track_characters_not_called_without_a_character_rate_limit_rule() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 10,
                },
            ),
        },
    };

    let messages = Arc::new(MockActivityRecorder::default());
    let characters = Arc::new(MockCharacterActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        messages.clone(),
        characters.clone(),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    assert_eq!(messages.recorded.lock().unwrap().len(), 1);
    assert!(
        characters.recorded.lock().unwrap().is_empty(),
        "the character log must not be written without a character rule"
    );
}

/// The line log is written only when a rule asks about lines, and what it is
/// told is how many lines this message took with the configured wrap width.
#[tokio::test]
async fn test_track_lines_called_when_line_rate_limit_rule_configured() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                line_count: 30,
                time_window_minutes: 10,
                chars_per_line: 10,
            }),
        },
    };

    let messages = Arc::new(MockActivityRecorder::default());
    let lines = Arc::new(MockLineActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        messages.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        lines.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg_time = Utc::now();
    // One hard line break, and a 25-character line that wraps into three at a
    // width of 10.
    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "short\n0123456789012345678901234".to_string(),
        attachment: None,
        timestamp: msg_time,
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    let recorded = lines.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1, "the line log must be written once");
    assert_eq!(recorded[0].0, 10, "group_id must match");
    assert_eq!(recorded[0].1, 42, "user_id must match");
    assert_eq!(recorded[0].2, msg_time, "the message timestamp is the now");
    assert_eq!(
        recorded[0].3, 4,
        "one line plus a 25-character line wrapped"
    );
    assert_eq!(
        recorded[0].4,
        Duration::from_secs(10 * 60),
        "ttl is the window"
    );

    assert!(
        messages.recorded.lock().unwrap().is_empty(),
        "a line rule must not start message tracking"
    );
}

/// One counter serves the group, so two conditions disagreeing about the width
/// are counted with the widest of them.
#[tokio::test]
async fn test_track_lines_uses_the_widest_configured_wrap() {
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
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                    line_count: 30,
                    time_window_minutes: 5,
                    chars_per_line: 10,
                }),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                    line_count: 100,
                    time_window_minutes: 20,
                    chars_per_line: 40,
                }),
            },
        },
    ];

    let lines = Arc::new(MockLineActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        lines.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "0123456789012345678901234".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    let recorded = lines.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].3, 1, "25 characters fit on one 40-wide line");
    assert_eq!(
        recorded[0].4,
        Duration::from_secs(20 * 60),
        "ttl is the longest window"
    );
}

/// Attachments are not counted: a picture with no caption weighs nothing, even
/// though an empty text is technically one empty line.
#[tokio::test]
async fn test_track_lines_counts_nothing_for_a_captionless_attachment() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                line_count: 30,
                time_window_minutes: 10,
                chars_per_line: 40,
            }),
        },
    };

    let lines = Arc::new(MockLineActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        lines.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: String::new(),
        attachment: Some(MessageAttachment::Image),
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    let recorded = lines.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].3, 0, "a caption-less picture is no lines");
}

/// A group with no line rule pays nothing for line tracking.
#[tokio::test]
async fn test_track_lines_not_called_without_a_line_rate_limit_rule() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
                max_lines: 5,
                chars_per_line: 40,
            }),
        },
    };

    let lines = Arc::new(MockLineActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        lines.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "a\nb".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    assert!(
        lines.recorded.lock().unwrap().is_empty(),
        "measuring one message's shape must not start line tracking"
    );
}

/// A zero window disables the condition, so nothing needs recording — same as
/// the message rate limit.
#[tokio::test]
async fn test_track_characters_not_called_when_window_is_zero() {
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsCharacterRateLimit(
                AuthorHitsCharacterRateLimit {
                    character_count: 2000,
                    time_window_minutes: 0,
                },
            ),
        },
    };

    let characters = Arc::new(MockCharacterActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(MockActivityRecorder::default()),
        characters.clone(),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    })
    .await
    .unwrap();

    assert!(characters.recorded.lock().unwrap().is_empty());
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
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["badword".to_string()],
                }),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 40,
                }),
            },
        },
    ];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        author_name: String::new(),
        text: "clean message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    // When there are no RateLimit rules, record_message must NOT be called
    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_message must not be called when group has no rate limit rules"
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
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules: vec![],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_message must not be called when group has empty rules"
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 0,
                },
            ),
        },
    }];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    app.process_group_message(msg).await.unwrap();

    assert!(
        recorder.recorded.lock().unwrap().is_empty(),
        "record_message must not be called when time_window_minutes is 0"
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 5,
                    time_window_minutes: 120, // exceeds 60 min ceiling
                },
            ),
        },
    }];

    let recorder = Arc::new(MockActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        recorder.clone(),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 55,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
                actions: vec![
                    ModerationAction::ModerateMessage(ModerateMessage {}),
                    ModerationAction::KickAuthor(KickAuthor {
                        delete_all_messages: false,
                    }),
                ],
                condition: ModerationCondition::AuthorHitsModerationRateLimit(
                    AuthorHitsModerationRateLimit {
                        message_count: 2,
                        time_window_minutes: 60,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["spam".to_string()],
                }),
            },
        },
    ];

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let kicked_members = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let activity_repo = Arc::new(InMemoryUserMessageActivityRepository::new());
    let moderation_activity_repo = Arc::new(InMemoryUserModerationActivityRepository::new());

    let app = MessageModerationApplication::new(
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
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        moderation_activity_repo,
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let make_msg = |msg_id: i64, text: &str| GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: msg_id,
        author_id: 777,
        author_name: String::new(),
        text: text.to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
        vec![
            ModerationAction::ModerateMessage(ModerateMessage {}),
            ModerationAction::KickAuthor(KickAuthor {
                delete_all_messages: false,
            }),
        ]
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
                actions: vec![ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                })],
                condition: ModerationCondition::AuthorHitsModerationRateLimit(
                    AuthorHitsModerationRateLimit {
                        message_count: 5,
                        time_window_minutes: 30,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["badword".to_string()],
                }),
            },
        },
    ];

    let mod_recorder = Arc::new(MockModerationActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        mod_recorder.clone(),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let clean_msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "clean message".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
        author_name: String::new(),
        text: "contains badword here".to_string(),
        attachment: None,
        timestamp: bad_time,
        author_joined_at: None,
        is_edit: false,
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
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
        },
    }];

    let mod_recorder = Arc::new(MockModerationActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(group),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        mod_recorder.clone(),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    let bad_msg = GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: "contains badword here".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
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
// Timed observer restrictions: what the bot owes the member afterwards.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Edits of an already posted message
//
// An edit is moderated like any other message, but it is not new traffic: the
// message it changes is already in the counters, so counting the edit too would
// let one message plus a few edits of it hit a limit its author never reached.
// ---------------------------------------------------------------------------

fn edit_test_group() -> Group {
    Group {
        id: 10,
        owner_id: 100,
        name: "Test Group".to_string(),
        notifications_enabled: true,
        dry_mode_enabled: false,
    }
}

fn edit_test_message(text: &str, is_edit: bool) -> GroupMessage {
    GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 1,
        author_id: 42,
        author_name: String::new(),
        text: text.to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit,
    }
}

#[tokio::test]
async fn test_editing_one_message_does_not_hit_the_message_rate_limit() {
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 3,
                    time_window_minutes: 1,
                },
            ),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(edit_test_message("first version", false))
        .await
        .unwrap();
    for version in 0..5 {
        app.process_group_message(edit_test_message(&format!("version {version}"), true))
            .await
            .unwrap();
    }

    assert!(
        deleted_messages.lock().unwrap().is_empty(),
        "one message and its edits must not reach a limit of 3 messages"
    );
}

#[tokio::test]
async fn test_edit_is_still_moderated_by_a_content_rule() {
    let rule = OwnedModerationRule {
        id: 1,
        rule: ModerationRule {
            actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
        },
    };

    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules: vec![rule],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(edit_test_message("edited into badword", true))
        .await
        .unwrap();

    assert_eq!(*deleted_messages.lock().unwrap(), vec![(10, 1)]);
}

#[tokio::test]
async fn test_edit_feeds_none_of_the_traffic_counters() {
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsMessageRateLimit(
                    AuthorHitsMessageRateLimit {
                        message_count: 5,
                        time_window_minutes: 1,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsCharacterRateLimit(
                    AuthorHitsCharacterRateLimit {
                        character_count: 2000,
                        time_window_minutes: 1,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 3,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
                    line_count: 30,
                    time_window_minutes: 1,
                    chars_per_line: 40,
                }),
            },
        },
    ];

    let messages = Arc::new(MockActivityRecorder::default());
    let characters = Arc::new(MockCharacterActivityRecorder::default());
    let lines = Arc::new(MockLineActivityRecorder::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules,
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        messages.clone(),
        characters.clone(),
        lines.clone(),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(edit_test_message("hello", false))
        .await
        .unwrap();
    assert_eq!(messages.recorded.lock().unwrap().len(), 1);
    assert_eq!(characters.recorded.lock().unwrap().len(), 1);
    assert_eq!(lines.recorded.lock().unwrap().len(), 1);

    app.process_group_message(edit_test_message("hello, and some more text", true))
        .await
        .unwrap();
    assert_eq!(
        messages.recorded.lock().unwrap().len(),
        1,
        "an edit must not be recorded as another message"
    );
    assert_eq!(
        characters.recorded.lock().unwrap().len(),
        1,
        "an edit must not be recorded as more characters written"
    );
    assert_eq!(
        lines.recorded.lock().unwrap().len(),
        1,
        "an edit must not be recorded as more lines taken"
    );
}

#[tokio::test]
async fn test_moderating_an_edit_does_not_join_the_moderated_tally() {
    let rules = vec![
        OwnedModerationRule {
            id: 1,
            rule: ModerationRule {
                actions: vec![ModerationAction::KickAuthor(KickAuthor {
                    delete_all_messages: false,
                })],
                condition: ModerationCondition::AuthorHitsModerationRateLimit(
                    AuthorHitsModerationRateLimit {
                        message_count: 5,
                        time_window_minutes: 30,
                    },
                ),
            },
        },
        OwnedModerationRule {
            id: 2,
            rule: ModerationRule {
                actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                condition: ModerationCondition::ContainsWords(ContainsWords {
                    keywords: vec!["badword".to_string()],
                }),
            },
        },
    ];

    let mod_recorder = Arc::new(MockModerationActivityRecorder::default());
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules,
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        mod_recorder.clone(),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(edit_test_message("edited into badword", true))
        .await
        .unwrap();

    assert_eq!(
        *deleted_messages.lock().unwrap(),
        vec![(10, 1)],
        "the edit itself is still moderated"
    );
    assert!(
        mod_recorder.recorded.lock().unwrap().is_empty(),
        "re-moderating an edited message must not count as another moderated message"
    );
}

fn app_with_observer_rule(
    duration_minutes: u32,
    restores: Arc<MockMemberRestoreRepository>,
) -> MessageModerationApplication {
    MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: 10,
                owner_id: 100,
                name: "Test Group".to_string(),
                notifications_enabled: false,
                dry_mode_enabled: false,
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes,
                    })],
                    condition: ModerationCondition::ContainsWords(ContainsWords {
                        keywords: vec!["danger".to_string()],
                    }),
                },
            }],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        restores,
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    )
}

fn triggering_message(timestamp: chrono::DateTime<Utc>) -> GroupMessage {
    GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id: 42,
        author_id: 777,
        author_name: String::new(),
        text: "Contains danger here".to_string(),
        attachment: None,
        timestamp,
        author_joined_at: None,
        is_edit: false,
    }
}

#[tokio::test]
async fn test_timed_observer_restriction_schedules_a_restore() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    let now = Utc::now();

    app_with_observer_rule(15, restores.clone())
        .process_group_message(triggering_message(now))
        .await
        .unwrap();

    assert_eq!(
        *restores.saved.lock().unwrap(),
        vec![(10, 777, now + chrono::Duration::minutes(15))]
    );
    assert!(restores.cancelled.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_indefinite_observer_restriction_cancels_any_scheduled_restore() {
    let restores = Arc::new(MockMemberRestoreRepository::default());

    app_with_observer_rule(0, restores.clone())
        .process_group_message(triggering_message(Utc::now()))
        .await
        .unwrap();

    assert!(restores.saved.lock().unwrap().is_empty());
    assert_eq!(*restores.cancelled.lock().unwrap(), vec![(10, 777)]);
}

#[tokio::test]
async fn test_dry_mode_schedules_nothing() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: 10,
                owner_id: 100,
                name: "Test Group".to_string(),
                notifications_enabled: false,
                dry_mode_enabled: true,
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::SetAuthorObserver(SetAuthorObserver {
                        duration_minutes: 15,
                    })],
                    condition: ModerationCondition::ContainsWords(ContainsWords {
                        keywords: vec!["danger".to_string()],
                    }),
                },
            }],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        restores.clone(),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(triggering_message(Utc::now()))
        .await
        .unwrap();

    assert!(restores.saved.lock().unwrap().is_empty());
    assert!(restores.cancelled.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_kicking_the_author_cancels_a_scheduled_restore() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: 10,
                owner_id: 100,
                name: "Test Group".to_string(),
                notifications_enabled: false,
                dry_mode_enabled: false,
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::KickAuthor(KickAuthor {
                        delete_all_messages: false,
                    })],
                    condition: ModerationCondition::ContainsWords(ContainsWords {
                        keywords: vec!["danger".to_string()],
                    }),
                },
            }],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        restores.clone(),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(triggering_message(Utc::now()))
        .await
        .unwrap();

    assert_eq!(*restores.cancelled.lock().unwrap(), vec![(10, 777)]);
}

#[tokio::test]
async fn test_failed_restore_bookkeeping_still_moderates_and_notifies() {
    let restores = Arc::new(MockMemberRestoreRepository {
        fail_writes: true,
        ..Default::default()
    });
    let deleted_messages = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: 10,
                owner_id: 100,
                name: "Test Group".to_string(),
                notifications_enabled: true,
                dry_mode_enabled: false,
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![
                        ModerationAction::SetAuthorObserver(SetAuthorObserver {
                            duration_minutes: 15,
                        }),
                        ModerationAction::ModerateMessage(ModerateMessage {}),
                    ],
                    condition: ModerationCondition::ContainsWords(ContainsWords {
                        keywords: vec!["danger".to_string()],
                    }),
                },
            }],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted_messages.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        restores,
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    );

    app.process_group_message(triggering_message(Utc::now()))
        .await
        .expect_err("the bookkeeping failure is still reported");

    assert_eq!(
        *deleted_messages.lock().unwrap(),
        vec![(10, 42)],
        "the offending message must still be moderated"
    );
    assert_eq!(
        notifications.lock().unwrap().len(),
        1,
        "the owner must still be notified"
    );
}

// ---------------------------------------------------------------------------
// FlaggedByOmniModeration
// ---------------------------------------------------------------------------

/// The classifier for groups whose rules never send a message to OpenAI.
pub struct UnusedAi;

#[async_trait]
impl OpenAi for UnusedAi {
    async fn verify(&self, _api_key: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn classify(
        &self,
        _api_key: &str,
        _text: &str,
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        panic!("OpenAI was asked about a message no rule sends to it")
    }
}

#[async_trait]
impl OpenRouter for UnusedAi {
    async fn verify_model(&self, _api_key: &str, _model: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn matches_instruction(
        &self,
        _api_key: &str,
        _model: &str,
        _instruction: &str,
        _author_name: &str,
        _text: &str,
        _context: &[crate::domain::moderator::ports::InstructionContextMessage],
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err> {
        panic!("OpenRouter was asked about a message no rule sends to it")
    }
}

/// Answers every message the same way and records the texts it was asked about.
pub struct ScriptedAi {
    answer: Result<OpenAiModerationResult, String>,
    texts: Mutex<Vec<String>>,
}

#[async_trait]
impl OpenAi for ScriptedAi {
    async fn verify(&self, _api_key: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn classify(
        &self,
        _api_key: &str,
        text: &str,
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        self.texts.lock().unwrap().push(text.to_string());
        self.answer.clone().map_err(Err::from)
    }
}

#[async_trait]
impl OpenRouter for ScriptedAi {
    async fn verify_model(&self, _api_key: &str, _model: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn matches_instruction(
        &self,
        _api_key: &str,
        _model: &str,
        _instruction: &str,
        _author_name: &str,
        text: &str,
        _context: &[crate::domain::moderator::ports::InstructionContextMessage],
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err> {
        self.texts.lock().unwrap().push(text.to_string());
        Ok(OpenRouterInstructionVerdict {
            matches: true,
            reason: "Promotes a coin.".to_string(),
        })
    }
}

fn app_with_openai_rule(
    openai: Arc<ScriptedAi>,
    deleted_messages: Arc<Mutex<Vec<(GroupId, i64)>>>,
    notifications: Arc<Mutex<Vec<(UserId, GroupId, Vec<ModerationAction>, String, String)>>>,
) -> MessageModerationApplication {
    let mut triggers = OpenAiCategoryTriggers::default();
    triggers.hate = CategoryTrigger::OpenAiDecides;
    MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                    condition: ModerationCondition::FlaggedByOmniModeration(
                        FlaggedByOmniModeration {
                            retry: Default::default(),
                            api_key: "sk-owner".to_string(),
                            triggers,
                        },
                    ),
                },
            }],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages,
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications,
            ..Default::default()
        }),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        openai.clone(),
        openai,
    )
}

fn hateful() -> OpenAiModerationResult {
    OpenAiModerationResult {
        flagged: [OpenAiCategory::Hate].into(),
        scores: [(OpenAiCategory::Hate, 0.9)].into(),
    }
}

#[tokio::test]
async fn test_a_message_openai_flags_is_deleted_and_reported() {
    let openai = Arc::new(ScriptedAi {
        answer: Ok(hateful()),
        texts: Mutex::new(Vec::new()),
    });
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_openai_rule(openai.clone(), deleted.clone(), notifications.clone());

    app.process_group_message(edit_test_message("something hateful", false))
        .await
        .unwrap();

    assert_eq!(*deleted.lock().unwrap(), vec![(10, 1)]);
    let notifications = notifications.lock().unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].4, "flagged by OpenAI Omni: hate (OpenAI)");
}

#[tokio::test]
async fn test_openai_being_down_moderates_nothing_and_is_not_an_error() {
    let openai = Arc::new(ScriptedAi {
        answer: Err("OpenAI is down".to_string()),
        texts: Mutex::new(Vec::new()),
    });
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_openai_rule(openai.clone(), deleted.clone(), notifications.clone());

    app.process_group_message(edit_test_message("something hateful", false))
        .await
        .expect("a provider failure must not fail the message");

    assert!(deleted.lock().unwrap().is_empty());
    assert!(notifications.lock().unwrap().is_empty());
    assert_eq!(openai.texts.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_an_edit_is_sent_to_openai_too() {
    // Editing a clean message into a hateful one is the obvious way around a
    // check that only looks at new messages.
    let openai = Arc::new(ScriptedAi {
        answer: Ok(hateful()),
        texts: Mutex::new(Vec::new()),
    });
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_openai_rule(
        openai.clone(),
        deleted.clone(),
        Arc::new(Mutex::new(Vec::new())),
    );

    app.process_group_message(edit_test_message("edited into something hateful", true))
        .await
        .unwrap();

    assert_eq!(
        *openai.texts.lock().unwrap(),
        vec!["edited into something hateful".to_string()]
    );
    assert_eq!(*deleted.lock().unwrap(), vec![(10, 1)]);
}

#[tokio::test]
async fn test_a_message_a_model_says_matches_the_instruction_is_deleted() {
    let openai = Arc::new(ScriptedAi {
        answer: Err("no moderation scripted".to_string()),
        texts: Mutex::new(Vec::new()),
    });
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let app = MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(edit_test_group()),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                    condition: ModerationCondition::FlaggedByOpenRouterInstruction(
                        FlaggedByOpenRouterInstruction {
                            retry: Default::default(),
                            api_key: "sk-owner".to_string(),
                            model: "openai/gpt-4o-mini".to_string(),
                            instruction: "Block crypto ads.".to_string(),
                            context_messages: 0,
                        },
                    ),
                },
            }],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages: deleted.clone(),
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier {
            notifications: notifications.clone(),
            ..Default::default()
        }),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        openai.clone(),
        openai.clone(),
    );

    app.process_group_message(edit_test_message("buy my coin", false))
        .await
        .unwrap();

    assert_eq!(*deleted.lock().unwrap(), vec![(10, 1)]);
    assert_eq!(
        notifications.lock().unwrap()[0].4,
        "openai/gpt-4o-mini: Promotes a coin."
    );
}

// ---------------------------------------------------------------------------
// Message history for FlaggedByOpenRouterInstruction's context
// ---------------------------------------------------------------------------

/// Says a text matches when it contains "spam".
struct SpamModel;

#[async_trait]
impl OpenAi for SpamModel {
    async fn verify(&self, _api_key: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn classify(
        &self,
        _api_key: &str,
        _text: &str,
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        panic!("OpenAI was asked about a message no rule sends to it")
    }
}

#[async_trait]
impl OpenRouter for SpamModel {
    async fn matches_instruction(
        &self,
        _api_key: &str,
        _model: &str,
        _instruction: &str,
        _author_name: &str,
        text: &str,
        _context: &[crate::domain::moderator::ports::InstructionContextMessage],
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err> {
        Ok(OpenRouterInstructionVerdict {
            matches: text.contains("spam"),
            reason: "Spam.".to_string(),
        })
    }

    async fn verify_model(&self, _api_key: &str, _model: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }
}

fn history_app(
    context_messages: u32,
    dry_mode_enabled: bool,
    history: Arc<InMemoryGroupMessageHistoryRepository>,
) -> MessageModerationApplication {
    let model = Arc::new(SpamModel);
    MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                dry_mode_enabled,
                ..edit_test_group()
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                    condition: ModerationCondition::FlaggedByOpenRouterInstruction(
                        FlaggedByOpenRouterInstruction {
                            retry: Default::default(),
                            api_key: "sk-owner".to_string(),
                            model: "openai/gpt-4o-mini".to_string(),
                            instruction: "Block spam.".to_string(),
                            context_messages,
                        },
                    ),
                },
            }],
        }),
        Arc::new(MockGroupModerator::default()),
        Arc::new(MockModerationNotifier::default()),
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        Arc::new(InMemoryGroupMessageActivityRepository::new()),
        Arc::new(InMemoryGroupCharacterActivityRepository::new()),
        Arc::new(InMemoryGroupLineActivityRepository::new()),
        history,
        Arc::new(MockMemberRestoreRepository::default()),
        model.clone(),
        model,
    )
}

fn history_message(message_id: MessageId, text: &str, is_edit: bool) -> GroupMessage {
    GroupMessage {
        message_id,
        ..edit_test_message(text, is_edit)
    }
}

/// The texts the history keeps for group 10, oldest first.
async fn kept_texts(history: &InMemoryGroupMessageHistoryRepository) -> Vec<String> {
    use crate::domain::moderator::ports::GroupMessageHistoryRepository;
    history
        .messages_before(&10, &0, 100, Utc::now())
        .await
        .unwrap()
        .into_iter()
        .map(|message| message.text)
        .collect()
}

#[tokio::test]
async fn test_messages_left_standing_are_kept_and_deleted_ones_are_not() {
    let history = Arc::new(InMemoryGroupMessageHistoryRepository::new());
    let app = history_app(2, false, history.clone());

    for (id, text) in [(1, "hello"), (2, "spam"), (3, "how are you"), (4, "fine")] {
        app.process_group_message(history_message(id, text, false))
            .await
            .unwrap();
    }

    // Only as many as the rule reads.
    assert_eq!(kept_texts(&history).await, vec!["how are you", "fine"]);
}

#[tokio::test]
async fn test_in_dry_mode_a_matching_message_stays_in_the_chat_and_in_the_history() {
    let history = Arc::new(InMemoryGroupMessageHistoryRepository::new());
    let app = history_app(5, true, history.clone());

    for (id, text) in [(1, "hello"), (2, "spam")] {
        app.process_group_message(history_message(id, text, false))
            .await
            .unwrap();
    }

    assert_eq!(kept_texts(&history).await, vec!["hello", "spam"]);
}

#[tokio::test]
async fn test_an_edit_replaces_the_kept_text_and_an_edit_deleted_is_forgotten() {
    let history = Arc::new(InMemoryGroupMessageHistoryRepository::new());
    let app = history_app(5, false, history.clone());

    for message in [
        history_message(1, "hello", false),
        history_message(2, "bye", false),
        history_message(1, "hello there", true),
        history_message(2, "bye, buy spam", true),
    ] {
        app.process_group_message(message).await.unwrap();
    }

    assert_eq!(kept_texts(&history).await, vec!["hello there"]);
}

#[tokio::test]
async fn test_nothing_is_kept_while_no_rule_reads_earlier_messages() {
    let history = Arc::new(InMemoryGroupMessageHistoryRepository::new());
    let app = history_app(0, false, history.clone());

    app.process_group_message(history_message(1, "hello", false))
        .await
        .unwrap();

    assert!(kept_texts(&history).await.is_empty());
    assert_eq!(history.active_group_count(), 0);
}

// ---------------------------------------------------------------------------
// Group-wide rate limits
// ---------------------------------------------------------------------------

/// The three group-wide counters, kept by the test so it can read them back.
#[derive(Default)]
struct GroupCounters {
    messages: Arc<InMemoryGroupMessageActivityRepository>,
    characters: Arc<InMemoryGroupCharacterActivityRepository>,
    lines: Arc<InMemoryGroupLineActivityRepository>,
}

fn group_rate_limit_app(
    condition: ModerationCondition,
    counters: &GroupCounters,
    user_messages: Arc<InMemoryUserMessageActivityRepository>,
    deleted_messages: Arc<Mutex<Vec<(GroupId, MessageId)>>>,
) -> MessageModerationApplication {
    MessageModerationApplication::new(
        Arc::new(MockModerationRepository {
            group: Some(Group {
                id: 10,
                owner_id: 100,
                name: "Test Group".to_string(),
                notifications_enabled: false,
                dry_mode_enabled: false,
            }),
            rules: vec![OwnedModerationRule {
                id: 1,
                rule: ModerationRule {
                    actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
                    condition,
                },
            }],
        }),
        Arc::new(MockGroupModerator {
            deleted_messages,
            ..Default::default()
        }),
        Arc::new(MockModerationNotifier::default()),
        user_messages,
        Arc::new(InMemoryUserCharacterActivityRepository::new()),
        Arc::new(InMemoryUserLineActivityRepository::new()),
        Arc::new(InMemoryUserModerationActivityRepository::new()),
        counters.messages.clone(),
        counters.characters.clone(),
        counters.lines.clone(),
        Arc::new(InMemoryGroupMessageHistoryRepository::new()),
        Arc::new(MockMemberRestoreRepository::default()),
        Arc::new(UnusedAi),
        Arc::new(UnusedAi),
    )
}

fn group_message_from(
    message_id: i64,
    author_id: i64,
    text: &str,
    timestamp: DateTime<Utc>,
) -> GroupMessage {
    GroupMessage {
        group: MessengerGroup {
            id: 10,
            name: "Test Group".to_string(),
        },
        message_id,
        author_id,
        author_name: String::new(),
        text: text.to_string(),
        attachment: None,
        timestamp,
        author_joined_at: None,
        is_edit: false,
    }
}

/// Three members posting one message each reach a group limit of three that
/// none of them comes near alone: the message that reaches it is deleted, the
/// ones before it are not.
#[tokio::test]
async fn test_group_message_rate_limit_counts_every_member() {
    let counters = GroupCounters::default();
    let user_messages = Arc::new(InMemoryUserMessageActivityRepository::new());
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let app = group_rate_limit_app(
        ModerationCondition::GroupHitsMessageRateLimit(GroupHitsMessageRateLimit {
            message_count: 3,
            time_window_minutes: 1,
        }),
        &counters,
        user_messages.clone(),
        deleted.clone(),
    );

    let now = Utc::now();
    for (i, author) in [1, 2, 3].into_iter().enumerate() {
        app.process_group_message(group_message_from(
            i as i64 + 1,
            author,
            "hi",
            now + chrono::Duration::seconds(i as i64),
        ))
        .await
        .unwrap();
    }

    assert_eq!(*deleted.lock().unwrap(), vec![(10, 3)]);
    assert_eq!(
        user_messages
            .count_messages_since(&10, &1, now - chrono::Duration::minutes(1), now)
            .await
            .unwrap(),
        0,
        "a group limit must not start the per-author counter"
    );
}

/// Characters add up across members: two six-character messages from
/// different authors reach a group limit of ten.
#[tokio::test]
async fn test_group_character_rate_limit_counts_every_member() {
    let counters = GroupCounters::default();
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let app = group_rate_limit_app(
        ModerationCondition::GroupHitsCharacterRateLimit(GroupHitsCharacterRateLimit {
            character_count: 10,
            time_window_minutes: 1,
        }),
        &counters,
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        deleted.clone(),
    );

    let now = Utc::now();
    app.process_group_message(group_message_from(1, 1, "Привет", now))
        .await
        .unwrap();
    assert!(deleted.lock().unwrap().is_empty());
    app.process_group_message(group_message_from(2, 2, "Привет", now))
        .await
        .unwrap();
    assert_eq!(*deleted.lock().unwrap(), vec![(10, 2)]);
}

/// Lines add up across members, wrapped at the width the condition sets: two
/// members' 80-character lines are two lines each at 40 per line.
#[tokio::test]
async fn test_group_line_rate_limit_counts_every_member() {
    let counters = GroupCounters::default();
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let app = group_rate_limit_app(
        ModerationCondition::GroupHitsLineRateLimit(GroupHitsLineRateLimit {
            line_count: 4,
            time_window_minutes: 1,
            chars_per_line: 40,
        }),
        &counters,
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        deleted.clone(),
    );

    let now = Utc::now();
    let long_line = "a".repeat(80);
    app.process_group_message(group_message_from(1, 1, &long_line, now))
        .await
        .unwrap();
    assert!(deleted.lock().unwrap().is_empty());
    app.process_group_message(group_message_from(2, 2, &long_line, now))
        .await
        .unwrap();
    assert_eq!(*deleted.lock().unwrap(), vec![(10, 2)]);
}

/// An edit is the same message again, so it must not count toward the group
/// limit any more than toward the author's.
#[tokio::test]
async fn test_group_message_rate_limit_ignores_edits() {
    let counters = GroupCounters::default();
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let app = group_rate_limit_app(
        ModerationCondition::GroupHitsMessageRateLimit(GroupHitsMessageRateLimit {
            message_count: 2,
            time_window_minutes: 1,
        }),
        &counters,
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        deleted.clone(),
    );

    let now = Utc::now();
    app.process_group_message(group_message_from(1, 1, "hi", now))
        .await
        .unwrap();
    app.process_group_message(GroupMessage {
        is_edit: true,
        ..group_message_from(1, 1, "hi there", now)
    })
    .await
    .unwrap();

    assert!(deleted.lock().unwrap().is_empty());
}

/// A group limiting only its authors pays nothing for the group counters.
#[tokio::test]
async fn test_group_counters_not_written_without_a_group_rate_limit_rule() {
    let counters = GroupCounters::default();
    let app = group_rate_limit_app(
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 1,
        }),
        &counters,
        Arc::new(InMemoryUserMessageActivityRepository::new()),
        Arc::new(Mutex::new(Vec::new())),
    );

    let now = Utc::now();
    app.process_group_message(group_message_from(1, 1, "hi", now))
        .await
        .unwrap();

    assert_eq!(
        counters
            .messages
            .count_messages_since(&10, now - chrono::Duration::minutes(1), now)
            .await
            .unwrap(),
        0
    );
}
