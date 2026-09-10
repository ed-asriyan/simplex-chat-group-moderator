use super::*;
use crate::domain::moderator::ports::{
    GroupMessage, MessengerGroup, MessengerGroupId, UserId, UserModerationActivityRepository,
};
use crate::infrastructure::adapters::user_activity_repo_in_memory::InMemoryUserActivityRepository;
use crate::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;
use chrono::{DateTime, Utc};

#[test]
fn test_deserialize_rule_with_moderate_message_action() {
    let json = r#"{
        "action": {
            "type": "ModerateMessage"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["spam", "ad"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.action, ModerationAction::ModerateMessage);
    match rule.condition {
        ModerationCondition::ContainsBannedWords { keywords } => {
            assert_eq!(keywords, vec!["spam".to_string(), "ad".to_string()]);
        }
        _ => panic!("Expected ContainsBannedWords condition"),
    }
}

#[test]
fn test_deserialize_rule_with_kick_author_action() {
    let json = r#"{
        "action": {
            "type": "KickAuthor",
            "delete_messages": "TriggeredMessage"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );

    // Test with delete_messages: "None"
    let json_no_delete = r#"{
        "action": {
            "type": "KickAuthor",
            "delete_messages": "None"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_no_delete: ModerationRule = serde_json::from_str(json_no_delete).unwrap();
    assert_eq!(
        rule_no_delete.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        }
    );

    // Test with delete_messages: "AllMessages"
    let json_all = r#"{
        "action": {
            "type": "KickAuthor",
            "delete_messages": "AllMessages"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_all: ModerationRule = serde_json::from_str(json_all).unwrap();
    assert_eq!(
        rule_all.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        }
    );

    // Test with default when delete_messages is omitted
    let json_default = r#"{
        "action": {
            "type": "KickAuthor"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_default: ModerationRule = serde_json::from_str(json_default).unwrap();
    assert_eq!(
        rule_default.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
}

#[test]
fn test_deserialize_rule_with_set_author_observer_action() {
    let json = r#"{
        "action": {
            "type": "SetAuthorObserver",
            "delete_message": "TriggeredMessage"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.action,
        ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::TriggeredMessage,
        }
    );

    // Test with delete_messages: "None"
    let json_no_delete = r#"{
        "action": {
            "type": "SetAuthorObserver",
            "delete_message": "None"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_no_delete: ModerationRule = serde_json::from_str(json_no_delete).unwrap();
    assert_eq!(
        rule_no_delete.action,
        ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::None,
        }
    );

    // Test with default when delete_messages is omitted
    let json_default = r#"{
        "action": {
            "type": "SetAuthorObserver"
        },
        "condition": {
            "type": "ContainsBannedWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_default: ModerationRule = serde_json::from_str(json_default).unwrap();
    assert_eq!(
        rule_default.action,
        ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::TriggeredMessage,
        }
    );
}

#[test]
fn test_serialization_roundtrip() {
    let rule = ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        },
        condition: ModerationCondition::MatchesExactMessage {
            messages: vec!["banned message".to_string()],
            case_sensitive: false,
        },
    };

    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: ModerationRule = serde_json::from_str(&serialized).unwrap();
    assert_eq!(rule, deserialized);
}

#[tokio::test]
async fn test_should_moderate_returns_matching_action_and_reason() {
    let rules = vec![ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
        condition: ModerationCondition::ContainsBannedWords {
            keywords: vec!["danger".to_string()],
        },
    }];

    let msg = GroupMessage {
        text: "This is a danger message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let result = should_moderate(&msg, &rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        }
    );
    assert_eq!(m.reason, "blacklisted word: 'danger'");
}

#[tokio::test]
async fn test_stronger_rule_upgrades_action_and_subsumes_weaker_rule() {
    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["first".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["second".to_string()],
            },
        },
    ];

    let msg = GroupMessage {
        text: "first and second both match".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    // When ModerateMessage is first, KickAuthor is stronger (covers ModerateMessage),
    // so it checks the second rule and upgrades to KickAuthor { TriggeredMessage }.
    let result = should_moderate(&msg, &rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    assert_eq!(
        m.actions,
        vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ]
    );
    assert_eq!(
        m.reason,
        "blacklisted word: 'first', blacklisted word: 'second'"
    );

    // Reversed rules list: KickAuthor is evaluated first.
    // ModerateMessage is a subset of KickAuthor { TriggeredMessage }, so its condition is skipped!
    let reversed_rules = vec![rules[1].clone(), rules[0].clone()];
    let reversed_result = should_moderate(&msg, &reversed_rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(reversed_result.is_some());
    let rm = reversed_result.unwrap();
    assert_eq!(
        rm.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    // Reason only contains 'second' because the subset rule was skipped!
    assert_eq!(rm.reason, "blacklisted word: 'second'");
}

#[tokio::test]
async fn test_no_rules_match_returns_none() {
    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::ContainsBannedWords {
            keywords: vec!["banned".to_string()],
        },
    }];

    let msg = GroupMessage {
        text: "all good here".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    assert!(
        should_moderate(&msg, &rules, &repo, &mod_repo)
            .await
            .unwrap()
            .is_none()
    );
}

#[test]
fn test_deserialize_rate_limit_rule() {
    let json = r#"{
        "action": {
            "type": "KickAuthor",
            "delete_messages": "AllMessages"
        },
        "condition": {
            "type": "UserExceedsMessagesRateLimit",
            "message_count": 5,
            "time_window_minutes": 10
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        }
    );
    assert_eq!(
        rule.condition,
        ModerationCondition::UserExceedsMessagesRateLimit {
            message_count: 5,
            time_window_minutes: 10,
        }
    );
}

#[test]
fn test_deserialize_rate_limit_rule_aliases() {
    let json = r#"{
        "action": {
            "type": "ModerateMessage"
        },
        "condition": {
            "type": "RateLimit",
            "count": 3,
            "minutes": 2
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.condition,
        ModerationCondition::UserExceedsMessagesRateLimit {
            message_count: 3,
            time_window_minutes: 2,
        }
    );
}

#[test]
fn test_deserialize_moderation_rate_limit_rule_aliases() {
    let json = r#"{
        "action": {
            "type": "KickAuthor"
        },
        "condition": {
            "type": "UserExceededModerationRateLimit",
            "moderated_count": 4,
            "window_minutes": 30
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.condition,
        ModerationCondition::UserExceedsModerationRateLimit {
            message_count: 4,
            time_window_minutes: 30,
        }
    );
}

#[test]
fn test_rate_limit_serialization_roundtrip() {
    let rule = ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::UserExceedsMessagesRateLimit {
            message_count: 10,
            time_window_minutes: 5,
        },
    };
    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: ModerationRule = serde_json::from_str(&serialized).unwrap();
    assert_eq!(rule, deserialized);
}

struct MockActivityRepoForFilter {
    count: u32,
}

#[async_trait::async_trait]
impl UserActivityRepository for MockActivityRepoForFilter {
    async fn record_user_message(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _timestamp: DateTime<Utc>,
        _ttl: std::time::Duration,
    ) -> Result<(), Err> {
        Ok(())
    }

    async fn count_messages_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(self.count)
    }
}

#[async_trait::async_trait]
impl UserModerationActivityRepository for MockActivityRepoForFilter {
    async fn record_moderated_message(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _timestamp: DateTime<Utc>,
        _ttl: std::time::Duration,
    ) -> Result<(), Err> {
        Ok(())
    }

    async fn count_moderated_messages_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(self.count)
    }
}

#[tokio::test]
async fn test_should_moderate_with_rate_limit() {
    let rules = vec![ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        },
        condition: ModerationCondition::UserExceedsMessagesRateLimit {
            message_count: 5,
            time_window_minutes: 1,
        },
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        text: "hello".to_string(),
        timestamp: Utc::now(),
    };

    let repo_under_limit = MockActivityRepoForFilter { count: 4 };
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let res = should_moderate(&msg, &rules, &repo_under_limit, &mod_repo)
        .await
        .unwrap();
    assert!(res.is_none());

    let repo_at_limit = MockActivityRepoForFilter { count: 5 };
    let res = should_moderate(&msg, &rules, &repo_at_limit, &mod_repo)
        .await
        .unwrap();
    assert!(res.is_some());
    let m = res.unwrap();
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    assert_eq!(
        m.reason,
        "user exceeds messages rate limit: 5 messages in 1 min"
    );
}

#[tokio::test]
async fn test_should_moderate_with_moderation_rate_limit() {
    let rules = vec![ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        },
        condition: ModerationCondition::UserExceedsModerationRateLimit {
            message_count: 3,
            time_window_minutes: 60,
        },
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        text: "hello".to_string(),
        timestamp: Utc::now(),
    };

    let activity_repo = InMemoryUserActivityRepository::new();
    let mod_repo_under = MockActivityRepoForFilter { count: 2 };
    let res = should_moderate(&msg, &rules, &activity_repo, &mod_repo_under)
        .await
        .unwrap();
    assert!(res.is_none());

    let mod_repo_at = MockActivityRepoForFilter { count: 3 };
    let res = should_moderate(&msg, &rules, &activity_repo, &mod_repo_at)
        .await
        .unwrap();
    assert!(res.is_some());
    let m = res.unwrap();
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    assert_eq!(
        m.reason,
        "user exceeds moderation rate limit: 3 moderated messages in 60 min"
    );
}

#[tokio::test]
async fn test_kick_author_all_messages_covers_moderate_message_and_moderate_message_disappears() {
    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["spam".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["malware".to_string()],
            },
        },
    ];

    let msg = GroupMessage {
        text: "spam and malware in same message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    let res = should_moderate(&msg, &rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(res.is_some());
    let m = res.unwrap();
    // ModerateMessage is completely covered by KickAuthor { AllMessages }
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        }
    );
    // DeleteTriggeredMessage disappeared! Only KickAuthor { delete_all_messages: true } remains.
    assert_eq!(
        m.actions,
        vec![PlannedAction::KickAuthor {
            delete_all_messages: true
        }]
    );
}

#[tokio::test]
async fn test_independent_rules_combine_and_order_deletion_before_kick() {
    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["spam".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::None,
            },
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["kickme".to_string()],
            },
        },
    ];

    let msg = GroupMessage {
        text: "spam and kickme in same message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    let res = should_moderate(&msg, &rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(res.is_some());
    let m = res.unwrap();
    // Combined action: KickAuthor with TriggeredMessage
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    // Deletion MUST come before kicking:
    assert_eq!(
        m.actions,
        vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ]
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_with_prior_moderation_increments_count() {
    // User has 2 previous moderated messages in DB, limit is 3 in 60 min.
    // Rule 1: ModerateMessage on "badword"
    // Rule 2: KickAuthor on UserExceedsModerationRateLimit (limit: 3)
    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["badword".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::UserExceedsModerationRateLimit {
                message_count: 3,
                time_window_minutes: 60,
            },
        },
    ];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        text: "this has badword".to_string(),
        timestamp: Utc::now(),
    };

    let activity_repo = InMemoryUserActivityRepository::new();
    // 2 moderated messages in DB: alone, Rule 2 would NOT trigger (2 < 3).
    // But since Rule 1 matches ("badword"), effective count becomes 2 + 1 = 3 >= 3!
    let mod_repo = MockActivityRepoForFilter { count: 2 };
    let res = should_moderate(&msg, &rules, &activity_repo, &mod_repo)
        .await
        .unwrap();
    assert!(res.is_some());
    let m = res.unwrap();
    assert_eq!(
        m.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_is_order_independent() {
    // The ordinary moderation rule comes after the moderation rate-limit rule.
    // The current message must still count toward the rate limit.
    let rules = vec![
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::UserExceedsModerationRateLimit {
                message_count: 3,
                time_window_minutes: 60,
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::ContainsBannedWords {
                keywords: vec!["badword".to_string()],
            },
        },
    ];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        author_id: 2,
        text: "this has badword".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    };

    let activity_repo = InMemoryUserActivityRepository::new();
    let moderation_activity_repo = MockActivityRepoForFilter { count: 2 };
    let result = should_moderate(&msg, &rules, &activity_repo, &moderation_activity_repo)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        result.action(),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    assert!(result.reason.contains("user exceeds moderation rate limit"));
}
