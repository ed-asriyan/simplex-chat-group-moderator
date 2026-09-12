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
        "actions": [{ "type": "ModerateMessage" }],
        "condition": {
            "type": "ContainsWords",
            "keywords": ["spam", "ad"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.actions, vec![ModerationAction::ModerateMessage]);
    match rule.condition {
        ModerationCondition::ContainsWords { keywords } => {
            assert_eq!(keywords, vec!["spam".to_string(), "ad".to_string()]);
        }
        _ => panic!("Expected ContainsWords condition"),
    }
}

#[test]
fn test_deserialize_rule_with_kick_author_action() {
    let json = r#"{
        "actions": [{ "type": "KickAuthor", "delete_all_messages": true }],
        "condition": {
            "type": "ContainsWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: true
        }]
    );

    // The editor always sends the flag; an omitted one means the author's
    // history is left alone.
    let json_default = r#"{
        "actions": [{ "type": "KickAuthor" }],
        "condition": {
            "type": "ContainsWords",
            "keywords": ["malware"]
        }
    }"#;
    let rule_default: ModerationRule = serde_json::from_str(json_default).unwrap();
    assert_eq!(
        rule_default.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: false
        }]
    );
}

#[test]
fn test_deserialize_rule_with_set_author_observer_action() {
    let json = r#"{
        "actions": [{ "type": "SetAuthorObserver" }],
        "condition": {
            "type": "ContainsWords",
            "keywords": ["spam"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.actions,
        vec![ModerationAction::SetAuthorObserver {
            duration_minutes: 0
        }]
    );
}

#[test]
fn test_deserialize_rule_with_several_actions_keeps_their_order() {
    let json = r#"{
        "actions": [
            { "type": "SetAuthorObserver" },
            { "type": "ModerateMessage" }
        ],
        "condition": {
            "type": "ContainsWords",
            "keywords": ["spam"]
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.actions,
        vec![
            ModerationAction::SetAuthorObserver {
                duration_minutes: 0
            },
            ModerationAction::ModerateMessage
        ]
    );
}

#[test]
fn test_normalize_and_validate_rejects_a_rule_without_actions() {
    let mut rule = ModerationRule {
        actions: vec![],
        condition: ModerationCondition::ContainsWords {
            keywords: vec!["spam".to_string()],
        },
    };
    let err = rule.normalize_and_validate().unwrap_err().to_string();
    assert!(err.contains("no actions"), "unexpected error: {err}");
}

#[test]
fn test_normalize_and_validate_canonicalizes_the_action_list() {
    let mut rule = ModerationRule {
        actions: vec![
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
            ModerationAction::ModerateMessage,
            // Already implied by the kick, and listed twice on top of that.
            ModerationAction::SetAuthorObserver {
                duration_minutes: 0,
            },
            ModerationAction::SetAuthorObserver {
                duration_minutes: 0,
            },
        ],
        condition: ModerationCondition::ContainsWords {
            keywords: vec!["spam".to_string()],
        },
    };
    rule.normalize_and_validate().unwrap();
    assert_eq!(
        rule.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false
            }
        ]
    );
}

#[test]
fn test_serialization_roundtrip() {
    let rule = ModerationRule {
        actions: vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ],
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
        actions: vec![ModerationAction::KickAuthor {
            delete_all_messages: false,
        }],
        condition: ModerationCondition::ContainsWords {
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
        m.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: false,
        },]
    );
    assert_eq!(m.reasons, vec!["contains word: 'danger'".to_string()]);
}

#[tokio::test]
async fn test_stronger_rule_upgrades_action_and_subsumes_weaker_rule() {
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["first".to_string()],
            },
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::ContainsWords {
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
    // so it checks the second rule and upgrades to moderate-then-kick.
    let result = should_moderate(&msg, &rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    assert_eq!(
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false
            }
        ]
    );
    assert_eq!(
        m.reasons,
        vec![
            "contains word: 'first'".to_string(),
            "contains word: 'second'".to_string()
        ]
    );

    // Reversed rules list: KickAuthor is evaluated first.
    // ModerateMessage is already part of the second rule's plan, so its condition is skipped!
    let reversed_rules = vec![rules[1].clone(), rules[0].clone()];
    let reversed_result = should_moderate(&msg, &reversed_rules, &repo, &mod_repo)
        .await
        .unwrap();
    assert!(reversed_result.is_some());
    let rm = reversed_result.unwrap();
    assert_eq!(
        rm.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    // Reason only contains 'second' because the subset rule was skipped!
    assert_eq!(rm.reasons, vec!["contains word: 'second'".to_string()]);
}

#[tokio::test]
async fn test_no_rules_match_returns_none() {
    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::ContainsWords {
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
fn test_deserialize_message_rate_limit_rule() {
    let json = r#"{
        "actions": [{ "type": "KickAuthor", "delete_all_messages": true }],
        "condition": {
            "type": "AuthorHitsMessageRateLimit",
            "message_count": 5,
            "time_window_minutes": 10
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(
        rule.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: true
        }]
    );
    assert_eq!(
        rule.condition,
        ModerationCondition::AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 10,
        }
    );
}

#[test]
fn test_message_rate_limit_serialization_roundtrip() {
    let rule = ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsMessageRateLimit {
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
async fn test_should_moderate_with_message_rate_limit() {
    let rules = vec![ModerationRule {
        actions: vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ],
        condition: ModerationCondition::AuthorHitsMessageRateLimit {
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
        author_joined_at: None,
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
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    assert_eq!(
        m.reasons,
        vec!["author sent 5 messages in 1 min".to_string()]
    );
}

#[tokio::test]
async fn test_should_moderate_with_moderation_rate_limit() {
    let rules = vec![ModerationRule {
        actions: vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ],
        condition: ModerationCondition::AuthorHitsModerationRateLimit {
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
        author_joined_at: None,
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
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    assert_eq!(
        m.reasons,
        vec!["author had 3 messages moderated in 60 min".to_string()]
    );
}

#[tokio::test]
async fn test_kick_author_all_messages_covers_moderate_message_and_moderate_message_disappears() {
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["spam".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: true,
            }],
            condition: ModerationCondition::ContainsWords {
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
        m.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: true,
        },]
    );
    // ModerateMessage disappeared! Only KickAuthor { delete_all_messages: true } remains.
    assert_eq!(
        m.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: true
        }]
    );
}

#[tokio::test]
async fn test_independent_rules_combine_and_order_deletion_before_kick() {
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["spam".to_string()],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: ModerationCondition::ContainsWords {
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
    // Combined plan: the message is moderated, then the author is kicked.
    assert_eq!(
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    // Deletion MUST come before kicking:
    assert_eq!(
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false
            }
        ]
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_with_prior_moderation_increments_count() {
    // User has 2 previous moderated messages in DB, limit is 3 in 60 min.
    // Rule 1: ModerateMessage on "badword"
    // Rule 2: KickAuthor on AuthorHitsModerationRateLimit (limit: 3)
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
                keywords: vec!["badword".to_string()],
            },
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::AuthorHitsModerationRateLimit {
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
        author_joined_at: None,
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
        m.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_is_order_independent() {
    // The ordinary moderation rule comes after the moderation rate-limit rule.
    // The current message must still count toward the rate limit.
    let rules = vec![
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::AuthorHitsModerationRateLimit {
                message_count: 3,
                time_window_minutes: 60,
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords {
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
        result.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    assert!(
        result
            .reasons
            .iter()
            .any(|r| r.contains("messages moderated in"))
    );
}

// ---------------------------------------------------------------------------
// Composite conditions
// ---------------------------------------------------------------------------

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords {
        keywords: vec![keyword.to_string()],
    }
}

fn rule_with(condition: ModerationCondition) -> Vec<ModerationRule> {
    vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
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
        conditions: vec![words("alpha"), words("beta")],
    };
    assert!(matched("alpha only", condition.clone()).await.is_none());
    assert!(matched("beta only", condition.clone()).await.is_none());

    let both = matched("alpha and beta", condition).await.unwrap();
    // Every child contributes its own reason, so the owner sees what combined.
    assert_eq!(
        both.reasons,
        vec!["contains word: 'alpha' and contains word: 'beta'".to_string()]
    );
}

#[tokio::test]
async fn test_any_matches_when_one_child_matches() {
    let condition = ModerationCondition::Any {
        conditions: vec![words("alpha"), words("beta")],
    };
    assert!(matched("nothing here", condition.clone()).await.is_none());

    let hit = matched("only beta", condition).await.unwrap();
    assert_eq!(hit.reasons, vec!["contains word: 'beta'".to_string()]);
}

#[tokio::test]
async fn test_not_inverts_its_child() {
    let condition = ModerationCondition::Not {
        condition: Box::new(words("allowed")),
    };
    assert!(
        matched("this is allowed", condition.clone())
            .await
            .is_none()
    );

    let hit = matched("anything else", condition).await.unwrap();
    // The child did not match, so it produced no reason of its own.
    assert_eq!(
        hit.reasons,
        vec!["does not match: contains one of the listed words".to_string()]
    );
}

#[tokio::test]
async fn test_nested_tree_combines_all_three_operators() {
    // "(alpha or beta) and not exempt"
    let condition = ModerationCondition::All {
        conditions: vec![
            ModerationCondition::Any {
                conditions: vec![words("alpha"), words("beta")],
            },
            ModerationCondition::Not {
                condition: Box::new(words("exempt")),
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
    // The pre-pass has to see through the composites: the word condition sits
    // inside an `Any`, and the rate limit inside an `All`. Before condition
    // trees this was a flat "every other rule" scan.
    let rules = vec![
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::All {
                conditions: vec![
                    ModerationCondition::AuthorHitsModerationRateLimit {
                        message_count: 3,
                        time_window_minutes: 60,
                    },
                    ModerationCondition::Not {
                        condition: Box::new(words("exempt")),
                    },
                ],
            },
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::Any {
                conditions: vec![words("badword"), words("otherword")],
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
        result.actions,
        vec![
            ModerationAction::ModerateMessage,
            ModerationAction::KickAuthor {
                delete_all_messages: false,
            },
        ]
    );
    assert!(
        result
            .reasons
            .iter()
            .any(|r| r.contains("messages moderated in"))
    );
}

#[tokio::test]
async fn test_moderation_rate_limit_in_a_tree_ignores_its_own_rule() {
    // The only other condition in this rule is the word condition, and it does not
    // match. With nothing else moderating the message, the current message must
    // not be counted, so two prior strikes stay under the limit of three.
    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
                words("absent"),
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
        "actions": [{ "type": "ModerateMessage" }],
        "condition": {
            "type": "All",
            "conditions": [
                {
                    "type": "Any",
                    "conditions": [
                        { "type": "ContainsWords", "keywords": ["spam"] },
                        { "type": "MatchesRegex", "patterns": ["\\d{4,}"] }
                    ]
                },
                {
                    "type": "Not",
                    "condition": { "type": "ContainsWords", "keywords": ["exempt"] }
                }
            ]
        }
    }"#;

    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    let expected = ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::All {
            conditions: vec![
                ModerationCondition::Any {
                    conditions: vec![
                        words("spam"),
                        ModerationCondition::MatchesRegex {
                            patterns: vec![r"\d{4,}".to_string()],
                        },
                    ],
                },
                ModerationCondition::Not {
                    condition: Box::new(words("exempt")),
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
        "actions": [{ "type": "ModerateMessage" }],
        "condition": { "type": "ContainsWords", "keywords": ["spam"] }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.condition, words("spam"));
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
// where that last child is a words condition the owner added and left empty.
// What follows pins down what the bot does with it.
// ---------------------------------------------------------------------------

/// The decoded contents of the link's `rules` parameter, with the condition
/// type renamed to its current name (`ContainsBannedWords` at the time).
const EDITOR_LINK_RULES_JSON: &str = r#"[{
    "actions": [{ "type": "ModerateMessage" }],
    "condition": {
        "type": "All",
        "conditions": [
            { "type": "ContainsWords", "keywords": ["test"] },
            {
                "type": "Any",
                "conditions": [
                    { "type": "ContainsWords", "keywords": ["message"] },
                    { "type": "ContainsWords", "keywords": ["mac"] }
                ]
            },
            { "type": "ContainsWords", "keywords": [] }
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
    assert_eq!(rules[0].actions, vec![ModerationAction::ModerateMessage]);

    // Nothing about this tree is redundant in the structural sense, so
    // normalization leaves it exactly as the editor sent it. In particular the
    // empty words condition is *kept*: an empty keyword list is a valid
    // list, unlike an empty `All`, which collapses.
    assert_eq!(
        rules[0].condition,
        ModerationCondition::All {
            conditions: vec![
                words("test"),
                ModerationCondition::Any {
                    conditions: vec![words("message"), words("mac")],
                },
                ModerationCondition::ContainsWords { keywords: vec![] },
            ],
        }
    );
}

#[tokio::test]
async fn test_editor_link_rule_never_moderates_anything() {
    // The owner wrote "test AND (message OR mac)" and left a third, empty
    // condition behind. A words condition with no words matches nothing,
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
        "actions": [{ "type": "ModerateMessage" }],
        "condition": {
            "type": "All",
            "conditions": [
                { "type": "ContainsWords", "keywords": ["test"] },
                {
                    "type": "Any",
                    "conditions": [
                        { "type": "ContainsWords", "keywords": ["message"] },
                        { "type": "ContainsWords", "keywords": ["mac"] }
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
async fn test_an_empty_words_condition_matches_nothing_on_its_own() {
    // The root cause, isolated: this is why the `All` above can never pass.
    let rules = saved(
        r#"[{ "actions": [{ "type": "ModerateMessage" }],
              "condition": { "type": "ContainsWords", "keywords": [] } }]"#,
    );
    assert!(!moderates(&rules, "anything at all").await);
    assert!(!moderates(&rules, "").await);
}

#[tokio::test]
async fn test_an_empty_words_condition_is_harmless_inside_any() {
    // Same empty condition, other composite: `Any` ignores a child that never
    // matches, so only `All` turns it into a rule-killer.
    let rules = saved(
        r#"[{
            "actions": [{ "type": "ModerateMessage" }],
            "condition": {
                "type": "Any",
                "conditions": [
                    { "type": "ContainsWords", "keywords": ["spam"] },
                    { "type": "ContainsWords", "keywords": [] }
                ]
            }
        }]"#,
    );
    assert!(moderates(&rules, "this is spam").await);
    assert!(!moderates(&rules, "this is fine").await);
}

// ---------------------------------------------------------------------------
// Author join time
// ---------------------------------------------------------------------------

/// Evaluate `condition` against `text` from an author who joined
/// `joined_minutes_ago` minutes before the message (`None`: unknown, i.e. a
/// member who was in the group before the bot).
async fn matched_from(
    joined_minutes_ago: Option<i64>,
    text: &str,
    condition: ModerationCondition,
) -> Option<ModerationMatch> {
    let now = Utc::now();
    let msg = GroupMessage {
        text: text.to_string(),
        timestamp: now,
        author_joined_at: joined_minutes_ago.map(|m| now - chrono::Duration::minutes(m)),
        ..Default::default()
    };
    let repo = InMemoryUserActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(&msg, &rule_with(condition), &repo, &mod_repo)
        .await
        .unwrap()
}

fn joined_recently(time_window_minutes: u32) -> ModerationCondition {
    ModerationCondition::AuthorJoinedRecently {
        time_window_minutes,
    }
}

#[test]
fn test_joined_recently_wire_format_matches_the_editor_schema() {
    let json = r#"{
        "actions": [{ "type": "ModerateMessage" }],
        "condition": { "type": "AuthorJoinedRecently", "time_window_minutes": 10 }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.condition, joined_recently(10));
    let round_tripped: ModerationRule =
        serde_json::from_str(&serde_json::to_string(&rule).unwrap()).unwrap();
    assert_eq!(round_tripped.condition, rule.condition);
}

#[tokio::test]
async fn test_joined_recently_matches_only_newcomers() {
    let hit = matched_from(Some(3), "hello", joined_recently(10))
        .await
        .unwrap();
    assert_eq!(
        hit.reasons,
        vec!["author joined 3 min ago (less than 10 min)".to_string()]
    );

    assert!(
        matched_from(Some(30), "hello", joined_recently(10))
            .await
            .is_none()
    );
    assert!(
        matched_from(None, "hello", joined_recently(10))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn test_joined_recently_narrows_another_condition_inside_all() {
    // The intended use: "newcomers may not post links", everyone else may.
    let condition = ModerationCondition::All {
        conditions: vec![joined_recently(60), words("http")],
    };
    let hit = matched_from(Some(5), "see http://x", condition.clone())
        .await
        .unwrap();
    assert_eq!(
        hit.reasons,
        vec!["author joined 5 min ago (less than 60 min) and contains word: 'http'".to_string()]
    );

    assert!(
        matched_from(Some(5), "just saying hi", condition.clone())
            .await
            .is_none()
    );
    assert!(
        matched_from(Some(90), "see http://x", condition.clone())
            .await
            .is_none()
    );
    assert!(
        matched_from(None, "see http://x", condition)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn test_not_joined_recently_matches_members_from_before_the_bot() {
    let condition = ModerationCondition::Not {
        condition: Box::new(joined_recently(10)),
    };
    assert!(
        matched_from(Some(3), "hello", condition.clone())
            .await
            .is_none()
    );

    let hit = matched_from(None, "hello", condition).await.unwrap();
    assert_eq!(
        hit.reasons,
        vec!["does not match: author joined less than 10 min ago".to_string()]
    );
}
