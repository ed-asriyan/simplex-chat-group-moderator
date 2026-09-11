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

// ---------------------------------------------------------------------------
// Composite conditions
// ---------------------------------------------------------------------------

fn banned(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsBannedWords {
        keywords: vec![keyword.to_string()],
    }
}

fn rule_with(condition: ModerationCondition) -> Vec<ModerationRule> {
    vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition,
    }]
}

async fn matched(text: &str, condition: ModerationCondition) -> Option<ModerationMatch> {
    let msg = GroupMessage {
        text: text.to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(&msg, &rule_with(condition), &repo, &mod_repo)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_all_matches_only_when_every_child_matches() {
    let condition = ModerationCondition::All {
        conditions: vec![banned("alpha"), banned("beta")],
    };
    assert!(matched("alpha only", condition.clone()).await.is_none());
    assert!(matched("beta only", condition.clone()).await.is_none());

    let both = matched("alpha and beta", condition).await.unwrap();
    // Every child contributes its own reason, so the owner sees what combined.
    assert_eq!(
        both.reason,
        "blacklisted word: 'alpha' and blacklisted word: 'beta'"
    );
}

#[tokio::test]
async fn test_any_matches_when_one_child_matches() {
    let condition = ModerationCondition::Any {
        conditions: vec![banned("alpha"), banned("beta")],
    };
    assert!(matched("nothing here", condition.clone()).await.is_none());

    let hit = matched("only beta", condition).await.unwrap();
    assert_eq!(hit.reason, "blacklisted word: 'beta'");
}

#[tokio::test]
async fn test_not_inverts_its_child() {
    let condition = ModerationCondition::Not {
        condition: Box::new(banned("allowed")),
    };
    assert!(
        matched("this is allowed", condition.clone())
            .await
            .is_none()
    );

    let hit = matched("anything else", condition).await.unwrap();
    // The child did not match, so it produced no reason of its own.
    assert_eq!(hit.reason, "does not match: contains a blacklisted word");
}

#[tokio::test]
async fn test_nested_tree_combines_all_three_operators() {
    // "(alpha or beta) and not exempt"
    let condition = ModerationCondition::All {
        conditions: vec![
            ModerationCondition::Any {
                conditions: vec![banned("alpha"), banned("beta")],
            },
            ModerationCondition::Not {
                condition: Box::new(banned("exempt")),
            },
        ],
    };

    assert!(matched("nothing", condition.clone()).await.is_none());
    assert!(matched("alpha exempt", condition.clone()).await.is_none());
    assert!(matched("beta here", condition.clone()).await.is_some());
    assert!(matched("alpha here", condition).await.is_some());
}

#[tokio::test]
async fn test_empty_composites_never_match() {
    // Normalization rejects these before they can be stored, but an empty `All`
    // is vacuously true, so evaluation refuses it too rather than moderating
    // every message if one ever reaches here.
    assert!(
        matched("anything", ModerationCondition::All { conditions: vec![] })
            .await
            .is_none()
    );
    assert!(
        matched("anything", ModerationCondition::Any { conditions: vec![] })
            .await
            .is_none()
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_counts_the_current_message_from_inside_a_tree() {
    // The pre-pass has to see through the composites: the banned word sits
    // inside an `Any`, and the rate limit inside an `All`. Before condition
    // trees this was a flat "every other rule" scan.
    let rules = vec![
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: ModerationCondition::All {
                conditions: vec![
                    ModerationCondition::UserExceedsModerationRateLimit {
                        message_count: 3,
                        time_window_minutes: 60,
                    },
                    ModerationCondition::Not {
                        condition: Box::new(banned("exempt")),
                    },
                ],
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: ModerationCondition::Any {
                conditions: vec![banned("badword"), banned("otherword")],
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
    // Two earlier moderated messages plus this one reaches the limit of three.
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

#[tokio::test]
async fn test_moderation_rate_limit_in_a_tree_ignores_its_own_rule() {
    // The only other condition in this rule is the banned word, and it does not
    // match. With nothing else moderating the message, the current message must
    // not be counted, so two prior strikes stay under the limit of three.
    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::UserExceedsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
                banned("absent"),
            ],
        },
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        author_id: 2,
        text: "clean text".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    };

    let activity_repo = InMemoryUserActivityRepository::new();
    let moderation_activity_repo = MockActivityRepoForFilter { count: 2 };
    let result = should_moderate(&msg, &rules, &activity_repo, &moderation_activity_repo)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn test_composite_wire_format_matches_the_editor_schema() {
    // The editor builds these objects from `rules-schema.json`, so the field
    // names here are a contract with it: `conditions` (array) on All/Any and
    // `condition` (object) on Not.
    let json = r#"{
        "action": { "type": "ModerateMessage" },
        "condition": {
            "type": "All",
            "conditions": [
                {
                    "type": "Any",
                    "conditions": [
                        { "type": "ContainsBannedWords", "keywords": ["spam"] },
                        { "type": "MatchesRegex", "patterns": ["\\d{4,}"] }
                    ]
                },
                {
                    "type": "Not",
                    "condition": { "type": "ContainsBannedWords", "keywords": ["exempt"] }
                }
            ]
        }
    }"#;

    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    let expected = ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: ModerationCondition::All {
            conditions: vec![
                ModerationCondition::Any {
                    conditions: vec![
                        banned("spam"),
                        ModerationCondition::MatchesRegex {
                            patterns: vec![r"\d{4,}".to_string()],
                        },
                    ],
                },
                ModerationCondition::Not {
                    condition: Box::new(banned("exempt")),
                },
            ],
        },
    };
    assert_eq!(rule, expected);

    // And back out again, since the bot hands the same JSON to the editor.
    let round_tripped: ModerationRule =
        serde_json::from_str(&serde_json::to_string(&expected).unwrap()).unwrap();
    assert_eq!(round_tripped, expected);
}

#[test]
fn test_a_flat_condition_still_deserializes_unchanged() {
    // Links already in the wild carry a bare condition object, which is just a
    // tree of one node.
    let json = r#"{
        "action": { "type": "ModerateMessage" },
        "condition": { "type": "ContainsBannedWords", "keywords": ["spam"] }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.condition, banned("spam"));
}

// ---------------------------------------------------------------------------
// A rule as it actually arrives from the editor
//
// Source link (group 655398 on a local editor):
//   http://127.0.0.1:8080/#bot_id=655398&rules=NobwRAhgxgLglgewHZgFzhgTwA4FM1gCy
//   CAJrgE4Qy6G4DOdEA5vgL4A0YUyJc8yaDDnyowAQQA2EsJ25Je-JHTSgwWPAQDCyGBDhKAQhCRJ
//   cJAOoJyJZZwDWuTAHcrNlWvowwAXQ5CNomJImDJcPHyISir+ImDaSLr6dEYmZpbWtmAOzq7KqMB
//   gALb0jCw+fmrCWjp6hsamFrmh2S4Z7oXQ5b7sMdUJtcn1aU32jq1u+b6+rN5AA
//
// The hash decompresses to the JSON below: one rule whose condition is
//   All[ words("test"), Any[ words("message"), words("mac") ], words() ]
// where that last child is a banned-words condition the owner added and left
// empty. What follows pins down what the bot does with it.
// ---------------------------------------------------------------------------

/// The decoded contents of the link's `rules` parameter, verbatim.
const EDITOR_LINK_RULES_JSON: &str = r#"[{
    "action": { "type": "ModerateMessage" },
    "condition": {
        "type": "All",
        "conditions": [
            { "type": "ContainsBannedWords", "keywords": ["test"] },
            {
                "type": "Any",
                "conditions": [
                    { "type": "ContainsBannedWords", "keywords": ["message"] },
                    { "type": "ContainsBannedWords", "keywords": ["mac"] }
                ]
            },
            { "type": "ContainsBannedWords", "keywords": [] }
        ]
    }
}]"#;

/// Saving goes through `normalize_and_validate`, so a test that skips it would
/// be evaluating a tree the repository would never have stored.
fn saved(json: &str) -> Vec<ModerationRule> {
    let mut rules: Vec<ModerationRule> = serde_json::from_str(json).unwrap();
    for rule in &mut rules {
        rule.condition.normalize_and_validate().unwrap();
    }
    rules
}

async fn moderates(rules: &[ModerationRule], text: &str) -> bool {
    let msg = GroupMessage {
        text: text.to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(&msg, rules, &repo, &mod_repo)
        .await
        .unwrap()
        .is_some()
}

#[test]
fn test_editor_link_rules_survive_saving_unchanged() {
    let rules = saved(EDITOR_LINK_RULES_JSON);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].action, ModerationAction::ModerateMessage);

    // Nothing about this tree is redundant in the structural sense, so
    // normalization leaves it exactly as the editor sent it. In particular the
    // empty banned-words condition is *kept*: an empty keyword list is a valid
    // list, unlike an empty `All`, which collapses.
    assert_eq!(
        rules[0].condition,
        ModerationCondition::All {
            conditions: vec![
                banned("test"),
                ModerationCondition::Any {
                    conditions: vec![banned("message"), banned("mac")],
                },
                ModerationCondition::ContainsBannedWords { keywords: vec![] },
            ],
        }
    );
}

#[tokio::test]
async fn test_editor_link_rule_never_moderates_anything() {
    // The owner wrote "test AND (message OR mac)" and left a third, empty
    // condition behind. A banned-words condition with no words matches nothing,
    // and inside an `All` that one failing child is enough to sink every
    // message — including the ones the rule was plainly built to catch.
    let rules = saved(EDITOR_LINK_RULES_JSON);

    assert!(!moderates(&rules, "test message").await);
    assert!(!moderates(&rules, "test mac").await);
    assert!(!moderates(&rules, "this is a test message about a mac").await);

    // And of course nothing else matches either.
    assert!(!moderates(&rules, "test").await);
    assert!(!moderates(&rules, "message").await);
    assert!(!moderates(&rules, "nothing relevant").await);
}

#[tokio::test]
async fn test_the_same_rule_without_the_empty_condition_works_as_intended() {
    // Identical to the link, minus the empty condition. This is what the owner
    // meant, and it shows the `All`/`Any` nesting itself is fine: the empty
    // child is the whole difference.
    let json = r#"[{
        "action": { "type": "ModerateMessage" },
        "condition": {
            "type": "All",
            "conditions": [
                { "type": "ContainsBannedWords", "keywords": ["test"] },
                {
                    "type": "Any",
                    "conditions": [
                        { "type": "ContainsBannedWords", "keywords": ["message"] },
                        { "type": "ContainsBannedWords", "keywords": ["mac"] }
                    ]
                }
            ]
        }
    }]"#;
    let rules = saved(json);

    // Both branches of the `Any` satisfy the rule when "test" is also present.
    assert!(moderates(&rules, "test message").await);
    assert!(moderates(&rules, "test mac").await);

    // "test" alone fails the `Any`; "message"/"mac" alone fail the first child.
    assert!(!moderates(&rules, "just a test").await);
    assert!(!moderates(&rules, "a message").await);
    assert!(!moderates(&rules, "my mac").await);
    assert!(!moderates(&rules, "nothing relevant").await);
}

#[tokio::test]
async fn test_an_empty_banned_words_condition_matches_nothing_on_its_own() {
    // The root cause, isolated: this is why the `All` above can never pass.
    let rules = saved(
        r#"[{ "action": { "type": "ModerateMessage" },
              "condition": { "type": "ContainsBannedWords", "keywords": [] } }]"#,
    );
    assert!(!moderates(&rules, "anything at all").await);
    assert!(!moderates(&rules, "").await);
}

#[tokio::test]
async fn test_an_empty_banned_words_condition_is_harmless_inside_any() {
    // Same empty condition, other composite: `Any` ignores a child that never
    // matches, so only `All` turns it into a rule-killer.
    let rules = saved(
        r#"[{
            "action": { "type": "ModerateMessage" },
            "condition": {
                "type": "Any",
                "conditions": [
                    { "type": "ContainsBannedWords", "keywords": ["spam"] },
                    { "type": "ContainsBannedWords", "keywords": [] }
                ]
            }
        }]"#,
    );
    assert!(moderates(&rules, "this is spam").await);
    assert!(!moderates(&rules, "this is fine").await);
}
