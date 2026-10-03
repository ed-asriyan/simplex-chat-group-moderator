use super::*;
use crate::domain::moderator::ports::conditions::{
    AuthorHitsCharacterRateLimit, AuthorHitsLineRateLimit, AuthorHitsMessageRateLimit,
    AuthorHitsModerationRateLimit, AuthorJoinedRecently, ContainsImage, ContainsWords,
    FlaggedByOmniModeration, FlaggedByOpenRouterInstruction, IsBlank, MatchesExactMessage,
    MatchesRegex,
};
use crate::domain::moderator::ports::{
    CategoryTrigger, GroupMessage, KeyCheck, MessageAttachment, MessengerGroup, MessengerGroupId,
    OpenAi, OpenAiCategory, OpenAiCategoryTriggers, OpenAiModerationResult, OpenRouter,
    OpenRouterInstructionVerdict, UserCharacterActivityRepository, UserId,
    UserLineActivityRepository, UserMessageActivityRepository, UserModerationActivityRepository,
};
use crate::infrastructure::adapters::group_character_activity_repo_in_memory::InMemoryGroupCharacterActivityRepository;
use crate::infrastructure::adapters::group_line_activity_repo_in_memory::InMemoryGroupLineActivityRepository;
use crate::infrastructure::adapters::group_message_activity_repo_in_memory::InMemoryGroupMessageActivityRepository;
use crate::infrastructure::adapters::group_message_history_repo_in_memory::InMemoryGroupMessageHistoryRepository;
use crate::infrastructure::adapters::user_character_activity_repo_in_memory::InMemoryUserCharacterActivityRepository;
use crate::infrastructure::adapters::user_line_activity_repo_in_memory::InMemoryUserLineActivityRepository;
use crate::infrastructure::adapters::user_message_activity_repo_in_memory::InMemoryUserMessageActivityRepository;
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
        ModerationCondition::ContainsWords(ContainsWords { keywords }) => {
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
        condition: ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["spam".to_string()],
        }),
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
        condition: ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["spam".to_string()],
        }),
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
        condition: ModerationCondition::MatchesExactMessage(MatchesExactMessage {
            messages: vec!["banned message".to_string()],
            case_sensitive: false,
        }),
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
        condition: ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["danger".to_string()],
        }),
    }];

    let msg = GroupMessage {
        text: "This is a danger message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let result = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["first".to_string()],
            }),
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["second".to_string()],
            }),
        },
    ];

    let msg = GroupMessage {
        text: "first and second both match".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    // When ModerateMessage is first, KickAuthor is stronger (covers ModerateMessage),
    // so it checks the second rule and upgrades to moderate-then-kick.
    let result = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
    let reversed_result = should_moderate(
        &msg,
        &reversed_rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
        condition: ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["banned".to_string()],
        }),
    }];

    let msg = GroupMessage {
        text: "all good here".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    assert!(
        should_moderate(
            &msg,
            &rules,
            &ConditionPorts {
                activity_repo: &repo,
                character_activity_repo: &no_character_activity(),
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
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
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 10,
        })
    );
}

#[test]
fn test_message_rate_limit_serialization_roundtrip() {
    let rule = ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 10,
            time_window_minutes: 5,
        }),
    };
    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: ModerationRule = serde_json::from_str(&serialized).unwrap();
    assert_eq!(rule, deserialized);
}

#[test]
fn test_deserialize_character_rate_limit_rule() {
    let json = r#"{
        "actions": [{ "type": "ModerateMessage" }],
        "condition": {
            "type": "AuthorHitsCharacterRateLimit",
            "character_count": 2000,
            "time_window_minutes": 5
        }
    }"#;
    let rule: ModerationRule = serde_json::from_str(json).unwrap();
    assert_eq!(rule.actions, vec![ModerationAction::ModerateMessage]);
    assert_eq!(
        rule.condition,
        ModerationCondition::AuthorHitsCharacterRateLimit(AuthorHitsCharacterRateLimit {
            character_count: 2000,
            time_window_minutes: 5,
        })
    );
}

#[test]
fn test_character_rate_limit_serialization_roundtrip() {
    let rule = ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsCharacterRateLimit(
            AuthorHitsCharacterRateLimit {
                character_count: 500,
                time_window_minutes: 2,
            },
        ),
    };
    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: ModerationRule = serde_json::from_str(&serialized).unwrap();
    assert_eq!(rule, deserialized);
}

/// Most tests here exercise no character rate limit, so they hand
/// `should_moderate` an empty character log.
#[tokio::test]
async fn test_should_moderate_with_line_rate_limit() {
    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsLineRateLimit(AuthorHitsLineRateLimit {
            line_count: 30,
            time_window_minutes: 1,
            chars_per_line: 40,
        }),
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    // Five lines short of the limit.
    let under = InMemoryUserLineActivityRepository::new();
    under
        .record_lines(
            &1,
            &2,
            msg.timestamp,
            25,
            std::time::Duration::from_secs(60),
        )
        .await
        .unwrap();
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &under,
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert!(res.is_none());

    // A five-line message takes them to it.
    let at = InMemoryUserLineActivityRepository::new();
    at.record_lines(
        &1,
        &2,
        msg.timestamp,
        25,
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    at.record_lines(&1, &2, msg.timestamp, 5, std::time::Duration::from_secs(60))
        .await
        .unwrap();
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &at,
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        res.unwrap().reasons,
        vec!["author sent 30 lines in 1 min".to_string()]
    );
}

fn no_character_activity() -> InMemoryUserCharacterActivityRepository {
    InMemoryUserCharacterActivityRepository::new()
}

fn no_line_activity() -> InMemoryUserLineActivityRepository {
    InMemoryUserLineActivityRepository::new()
}

fn no_group_activity() -> InMemoryGroupMessageActivityRepository {
    InMemoryGroupMessageActivityRepository::new()
}

fn no_group_character_activity() -> InMemoryGroupCharacterActivityRepository {
    InMemoryGroupCharacterActivityRepository::new()
}

fn no_group_line_activity() -> InMemoryGroupLineActivityRepository {
    InMemoryGroupLineActivityRepository::new()
}

fn no_message_history() -> InMemoryGroupMessageHistoryRepository {
    InMemoryGroupMessageHistoryRepository::new()
}

struct MockActivityRepoForFilter {
    count: u32,
}

#[async_trait::async_trait]
impl UserMessageActivityRepository for MockActivityRepoForFilter {
    async fn record_message(
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
async fn test_should_moderate_with_character_rate_limit() {
    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsCharacterRateLimit(
            AuthorHitsCharacterRateLimit {
                character_count: 500,
                time_window_minutes: 1,
            },
        ),
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    // The author is 100 characters short of the limit.
    let under = InMemoryUserCharacterActivityRepository::new();
    under
        .record_characters(
            &1,
            &2,
            msg.timestamp,
            400,
            std::time::Duration::from_secs(60),
        )
        .await
        .unwrap();
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &under,
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert!(res.is_none());

    // One more long message takes them over it.
    let at = InMemoryUserCharacterActivityRepository::new();
    at.record_characters(
        &1,
        &2,
        msg.timestamp,
        400,
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    at.record_characters(
        &1,
        &2,
        msg.timestamp,
        100,
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &at,
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        res.unwrap().reasons,
        vec!["author sent 500 characters in 1 min".to_string()]
    );
}

/// The two rate limits read different logs: a member who posts constantly but
/// briefly trips the message limit, and one who posts a single essay trips the
/// character limit. Neither must see the other's traffic.
#[tokio::test]
async fn test_message_and_character_rate_limits_read_their_own_logs() {
    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let ttl = std::time::Duration::from_secs(60);

    // Ten one-character messages: ten messages, ten characters.
    let messages = InMemoryUserMessageActivityRepository::new();
    let characters = InMemoryUserCharacterActivityRepository::new();
    for _ in 0..10 {
        messages
            .record_message(&1, &2, msg.timestamp, ttl)
            .await
            .unwrap();
        characters
            .record_characters(&1, &2, msg.timestamp, 1, ttl)
            .await
            .unwrap();
    }

    let by_messages = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 10,
            time_window_minutes: 1,
        }),
    }];
    let by_characters = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::AuthorHitsCharacterRateLimit(
            AuthorHitsCharacterRateLimit {
                character_count: 10,
                time_window_minutes: 1,
            },
        ),
    }];

    // Ten messages hit the message limit of 10 and the character limit of 10
    // is reached too, since ten single-character messages are ten characters.
    assert!(
        should_moderate(
            &msg,
            &by_messages,
            &ConditionPorts {
                activity_repo: &messages,
                character_activity_repo: &characters,
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
        .is_some()
    );
    assert!(
        should_moderate(
            &msg,
            &by_characters,
            &ConditionPorts {
                activity_repo: &messages,
                character_activity_repo: &characters,
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
        .is_some()
    );

    // One 1000-character message: one message, a thousand characters. Only the
    // character limit fires; the message limit sees a single message.
    let messages = InMemoryUserMessageActivityRepository::new();
    let characters = InMemoryUserCharacterActivityRepository::new();
    messages
        .record_message(&1, &2, msg.timestamp, ttl)
        .await
        .unwrap();
    characters
        .record_characters(&1, &2, msg.timestamp, 1000, ttl)
        .await
        .unwrap();

    assert!(
        should_moderate(
            &msg,
            &by_messages,
            &ConditionPorts {
                activity_repo: &messages,
                character_activity_repo: &characters,
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        should_moderate(
            &msg,
            &by_characters,
            &ConditionPorts {
                activity_repo: &messages,
                character_activity_repo: &characters,
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
        .is_some()
    );
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
        condition: ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 5,
            time_window_minutes: 1,
        }),
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let repo_under_limit = MockActivityRepoForFilter { count: 4 };
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo_under_limit,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert!(res.is_none());

    let repo_at_limit = MockActivityRepoForFilter { count: 5 };
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo_at_limit,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
        condition: ModerationCondition::AuthorHitsModerationRateLimit(
            AuthorHitsModerationRateLimit {
                message_count: 3,
                time_window_minutes: 60,
            },
        ),
    }];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "hello".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo_under = MockActivityRepoForFilter { count: 2 };
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo_under,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap();
    assert!(res.is_none());

    let mod_repo_at = MockActivityRepoForFilter { count: 3 };
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo_at,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["spam".to_string()],
            }),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: true,
            }],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["malware".to_string()],
            }),
        },
    ];

    let msg = GroupMessage {
        text: "spam and malware in same message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["spam".to_string()],
            }),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["kickme".to_string()],
            }),
        },
    ];

    let msg = GroupMessage {
        text: "spam and kickme in same message".to_string(),
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();

    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
        },
        ModerationRule {
            actions: vec![
                ModerationAction::ModerateMessage,
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
            ],
            condition: ModerationCondition::AuthorHitsModerationRateLimit(
                AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
            ),
        },
    ];

    let msg = GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: "this has badword".to_string(),
        attachment: None,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    // 2 moderated messages in DB: alone, Rule 2 would NOT trigger (2 < 3).
    // But since Rule 1 matches ("badword"), effective count becomes 2 + 1 = 3 >= 3!
    let mod_repo = MockActivityRepoForFilter { count: 2 };
    let res = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
            condition: ModerationCondition::AuthorHitsModerationRateLimit(
                AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                },
            ),
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsWords(ContainsWords {
                keywords: vec!["badword".to_string()],
            }),
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

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    let moderation_activity_repo = MockActivityRepoForFilter { count: 2 };
    let result = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &moderation_activity_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
    // The keyword rule is skipped by the main loop (the kick covers its
    // action), but it is why this message counted, so the owner is told.
    assert_eq!(
        result.reasons,
        vec![
            "author had 3 messages moderated in 60 min".to_string(),
            "contains word: 'badword'".to_string(),
        ]
    );
}

// ---------------------------------------------------------------------------
// Composite conditions
// ---------------------------------------------------------------------------

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords(ContainsWords {
        keywords: vec![keyword.to_string()],
    })
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
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(
        &msg,
        &rule_with(condition),
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
                    ModerationCondition::AuthorHitsModerationRateLimit(
                        AuthorHitsModerationRateLimit {
                            message_count: 3,
                            time_window_minutes: 60,
                        },
                    ),
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

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    // Two earlier moderated messages plus this one reaches the limit of three.
    let moderation_activity_repo = MockActivityRepoForFilter { count: 2 };
    let result = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &moderation_activity_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
    assert_eq!(result.reasons.len(), 2);
    assert!(result.reasons[0].contains("messages moderated in"));
    // The reported case: the kick covers the word rule, which the owner would
    // otherwise never hear was why this message counted.
    assert_eq!(result.reasons[1], "contains word: 'badword'");
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
                ModerationCondition::AuthorHitsModerationRateLimit(AuthorHitsModerationRateLimit {
                    message_count: 3,
                    time_window_minutes: 60,
                }),
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

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    let moderation_activity_repo = MockActivityRepoForFilter { count: 2 };
    let result = should_moderate(
        &msg,
        &rules,
        &ConditionPorts {
            activity_repo: &activity_repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &moderation_activity_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
                        ModerationCondition::MatchesRegex(MatchesRegex {
                            patterns: vec![r"\d{4,}".to_string()],
                        }),
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
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(
        &msg,
        rules,
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
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
                ModerationCondition::ContainsWords(ContainsWords { keywords: vec![] }),
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
        attachment: None,
        timestamp: now,
        author_joined_at: joined_minutes_ago.map(|m| now - chrono::Duration::minutes(m)),
        is_edit: false,
        ..Default::default()
    };
    let repo = InMemoryUserMessageActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    should_moderate(
        &msg,
        &rule_with(condition),
        &ConditionPorts {
            activity_repo: &repo,
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &mod_repo,
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: &UnusedAi,
            openrouter: &UnusedAi,
        },
    )
    .await
    .unwrap()
}

fn joined_recently(time_window_minutes: u32) -> ModerationCondition {
    ModerationCondition::AuthorJoinedRecently(AuthorJoinedRecently {
        time_window_minutes,
    })
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

#[tokio::test]
async fn test_moderates_pictures_and_leaves_them_out_of_is_blank() {
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::ContainsImage(ContainsImage {}),
        },
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: ModerationCondition::IsBlank(IsBlank {}),
        },
    ];

    let message = |text: &str, attachment: Option<MessageAttachment>| GroupMessage {
        group: MessengerGroup {
            id: 1,
            name: "Test Group".to_string(),
        },
        message_id: 10,
        author_id: 2,
        author_name: String::new(),
        text: text.to_string(),
        attachment,
        timestamp: Utc::now(),
        author_joined_at: None,
        is_edit: false,
    };

    let activity_repo = InMemoryUserMessageActivityRepository::new();
    let moderation_repo = InMemoryUserModerationActivityRepository::new();
    let matched = async |message: &GroupMessage| {
        should_moderate(
            message,
            &rules,
            &ConditionPorts {
                activity_repo: &activity_repo,
                character_activity_repo: &no_character_activity(),
                line_activity_repo: &no_line_activity(),
                moderation_activity_repo: &moderation_repo,
                group_activity_repo: &no_group_activity(),
                group_character_activity_repo: &no_group_character_activity(),
                group_line_activity_repo: &no_group_line_activity(),
                message_history: &no_message_history(),
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
    };

    // A picture is caught by the picture condition, captioned or not.
    let captionless = message("", Some(MessageAttachment::Image));
    assert_eq!(
        matched(&captionless).await.unwrap().reasons,
        vec!["contains an image".to_string()]
    );
    let captioned = message("look at this", Some(MessageAttachment::Image));
    assert_eq!(
        matched(&captioned).await.unwrap().reasons,
        vec!["contains an image".to_string()]
    );

    // ...and never by "is empty or blank": a message that carries something is
    // not an empty message, however empty its caption is. Only the picture rule
    // reports, so a single reason means the blank rule stayed out of it.
    let blank_caption = message("   ", Some(MessageAttachment::Image));
    assert_eq!(
        matched(&blank_caption).await.unwrap().reasons,
        vec!["contains an image".to_string()]
    );
    let captionless_file = message("", Some(MessageAttachment::File));
    assert!(matched(&captionless_file).await.is_none());

    // A text message with nothing in it is still blank.
    let blank_text = message("   ", None);
    assert_eq!(
        matched(&blank_text).await.unwrap().reasons,
        vec!["empty message".to_string()]
    );
}

// ---------------------------------------------------------------------------
// FlaggedByOmniModeration
// ---------------------------------------------------------------------------

/// The provider for rule sets that never send a message to OpenAI or
/// OpenRouter: being asked at all is the bug.
pub struct UnusedAi;

#[async_trait::async_trait]
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

#[async_trait::async_trait]
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

/// Answers every message the same way and records what it was asked.
pub struct ScriptedAi {
    answer: Result<OpenAiModerationResult, String>,
    verdict: Result<OpenRouterInstructionVerdict, String>,
    calls: std::sync::Mutex<Vec<(String, String)>>,
    instruction_calls: std::sync::Mutex<Vec<(String, String, String, String)>>,
    contexts:
        std::sync::Mutex<Vec<Vec<crate::domain::moderator::ports::InstructionContextMessage>>>,
    judged_authors: std::sync::Mutex<Vec<String>>,
}

impl ScriptedAi {
    pub fn answering(answer: Result<OpenAiModerationResult, String>) -> Self {
        Self {
            answer,
            verdict: Err("no verdict scripted".to_string()),
            calls: std::sync::Mutex::new(Vec::new()),
            instruction_calls: std::sync::Mutex::new(Vec::new()),
            contexts: std::sync::Mutex::new(Vec::new()),
            judged_authors: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Answers every instruction the same way, with `reason` as its reason.
    pub fn judging(verdict: Result<bool, String>) -> Self {
        Self::judging_with(verdict, "Promotes a coin.")
    }

    pub fn judging_with(verdict: Result<bool, String>, reason: &str) -> Self {
        Self {
            verdict: verdict.map(|matches| OpenRouterInstructionVerdict {
                matches,
                reason: reason.to_string(),
            }),
            ..Self::answering(Err("no moderation scripted".to_string()))
        }
    }

    pub fn judged_authors(&self) -> Vec<String> {
        self.judged_authors.lock().unwrap().clone()
    }

    pub fn contexts(&self) -> Vec<Vec<crate::domain::moderator::ports::InstructionContextMessage>> {
        self.contexts.lock().unwrap().clone()
    }

    pub fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }

    /// (key, model, instruction, text) for every instruction asked.
    pub fn instruction_calls(&self) -> Vec<(String, String, String, String)> {
        self.instruction_calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl OpenAi for ScriptedAi {
    async fn verify(&self, _api_key: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn classify(
        &self,
        api_key: &str,
        text: &str,
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenAiModerationResult, Err> {
        self.calls
            .lock()
            .unwrap()
            .push((api_key.to_string(), text.to_string()));
        self.answer.clone().map_err(Err::from)
    }
}

#[async_trait::async_trait]
impl OpenRouter for ScriptedAi {
    async fn verify_model(&self, _api_key: &str, _model: &str) -> KeyCheck {
        panic!("no key is checked while moderating a message")
    }

    async fn matches_instruction(
        &self,
        api_key: &str,
        model: &str,
        instruction: &str,
        author_name: &str,
        text: &str,
        context: &[crate::domain::moderator::ports::InstructionContextMessage],
        _retry: &crate::domain::moderator::ports::ApiRetry,
    ) -> Result<OpenRouterInstructionVerdict, Err> {
        self.contexts.lock().unwrap().push(context.to_vec());
        self.judged_authors
            .lock()
            .unwrap()
            .push(author_name.to_string());
        self.instruction_calls.lock().unwrap().push((
            api_key.to_string(),
            model.to_string(),
            instruction.to_string(),
            text.to_string(),
        ));
        self.verdict.clone().map_err(Err::from)
    }
}

/// OpenAI flagged hate, and scored it 0.9.
fn hateful() -> OpenAiModerationResult {
    OpenAiModerationResult {
        flagged: [OpenAiCategory::Hate].into(),
        scores: [(OpenAiCategory::Hate, 0.9)].into(),
    }
}

/// Matches whatever OpenAI flags as hate.
fn openai_hate(api_key: &str) -> ModerationCondition {
    let mut triggers = OpenAiCategoryTriggers::default();
    triggers.hate = CategoryTrigger::OpenAiDecides;
    ModerationCondition::FlaggedByOmniModeration(FlaggedByOmniModeration {
        retry: Default::default(),
        api_key: api_key.to_string(),
        triggers,
    })
}

async fn matched_with(
    openai: &ScriptedAi,
    msg: &GroupMessage,
    rules: &[ModerationRule],
) -> Option<ModerationMatch> {
    should_moderate(
        msg,
        rules,
        &ConditionPorts {
            activity_repo: &InMemoryUserMessageActivityRepository::new(),
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &InMemoryUserModerationActivityRepository::new(),
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: &no_message_history(),
            openai: openai,
            openrouter: openai,
        },
    )
    .await
    .expect("an OpenAI answer, or its absence, is never an evaluation error")
}

fn text_message(text: &str) -> GroupMessage {
    GroupMessage {
        text: text.to_string(),
        ..Default::default()
    }
}

#[test]
fn test_openai_condition_wire_format_matches_the_editor_schema() {
    let json = r#"{
        "type": "FlaggedByOmniModeration",
        "api_key": "sk-proj-abc",
        "hate": "openai",
        "violence": 80,
        "sexual": "off"
    }"#;
    let condition: ModerationCondition = serde_json::from_str(json).unwrap();
    let ModerationCondition::FlaggedByOmniModeration(FlaggedByOmniModeration {
        api_key,
        triggers,
        ..
    }) = &condition
    else {
        panic!("expected FlaggedByOmniModeration, got {condition:?}");
    };
    assert_eq!(api_key, "sk-proj-abc");
    assert_eq!(triggers.hate, CategoryTrigger::OpenAiDecides);
    assert_eq!(triggers.violence, CategoryTrigger::MinScorePercent(80));
    assert_eq!(triggers.sexual, CategoryTrigger::Off);
    // Categories the JSON leaves out are off.
    assert_eq!(triggers.harassment, CategoryTrigger::Off);

    // The key travels in the editor link as it is, and every category is
    // written out so the editor shows all thirteen.
    let written = serde_json::to_value(&condition).unwrap();
    assert_eq!(written["type"], "FlaggedByOmniModeration");
    assert_eq!(written["api_key"], "sk-proj-abc");
    assert_eq!(written["hate"], "openai");
    assert_eq!(written["violence"], 80);
    assert_eq!(written["self_harm_instructions"], "off");
    assert_eq!(written.as_object().unwrap().len(), 2 + 13 + 2);
    assert_eq!(written["max_attempts"], 3);
    assert_eq!(written["retry_delay_seconds"], 1);

    let reread: ModerationCondition = serde_json::from_value(written).unwrap();
    assert_eq!(reread, condition);
}

#[test]
fn test_openai_condition_without_a_key_or_with_a_bad_trigger_does_not_parse() {
    for json in [
        r#"{ "type": "FlaggedByOmniModeration", "hate": "openai" }"#,
        r#"{ "type": "FlaggedByOmniModeration", "api_key": "sk", "hate": "maybe" }"#,
        r#"{ "type": "FlaggedByOmniModeration", "api_key": "sk", "hate": 300 }"#,
    ] {
        assert!(
            serde_json::from_str::<ModerationCondition>(json).is_err(),
            "{json} should not parse"
        );
    }
}

#[tokio::test]
async fn test_openai_verdict_moderates_with_its_reason() {
    let openai = ScriptedAi::answering(Ok(hateful()));

    let hit = matched_with(
        &openai,
        &text_message("some text"),
        &rule_with(openai_hate("sk-one")),
    )
    .await
    .unwrap();

    assert_eq!(hit.actions, vec![ModerationAction::ModerateMessage]);
    assert_eq!(
        hit.reasons,
        vec!["flagged by OpenAI Omni: hate (OpenAI)".to_string()]
    );
}

#[tokio::test]
async fn test_openai_verdict_that_trips_no_trigger_does_not_match() {
    let openai = ScriptedAi::answering(Ok(OpenAiModerationResult {
        flagged: [OpenAiCategory::Violence].into(),
        scores: [(OpenAiCategory::Violence, 0.99)].into(),
    }));

    let hit = matched_with(
        &openai,
        &text_message("some text"),
        &rule_with(openai_hate("sk-one")),
    )
    .await;

    assert!(hit.is_none());
}

#[tokio::test]
async fn test_openai_is_asked_with_the_condition_key_and_the_message_text_as_is() {
    let openai = ScriptedAi::answering(Ok(OpenAiModerationResult::default()));
    let text = "  Привет,\nмир  ";

    matched_with(
        &openai,
        &text_message(text),
        &rule_with(openai_hate("sk-one")),
    )
    .await;

    assert_eq!(
        openai.calls(),
        vec![("sk-one".to_string(), text.to_string())]
    );
}

#[tokio::test]
async fn test_openai_is_not_asked_about_a_message_without_text() {
    let openai = ScriptedAi::answering(Ok(hateful()));
    let rules = rule_with(openai_hate("sk-one"));

    let blank = text_message(" \n\t ");
    let captionless_picture = GroupMessage {
        attachment: Some(MessageAttachment::Image),
        ..Default::default()
    };
    assert!(matched_with(&openai, &blank, &rules).await.is_none());
    assert!(
        matched_with(&openai, &captionless_picture, &rules)
            .await
            .is_none()
    );
    assert!(openai.calls().is_empty());
}

#[tokio::test]
async fn test_openai_failure_reads_as_no_match_and_other_rules_still_apply() {
    let openai = ScriptedAi::answering(Err("OpenAI is down".to_string()));
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openai_hate("sk-one"),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: words("spam"),
        },
    ];

    let hit = matched_with(&openai, &text_message("spam"), &rules)
        .await
        .unwrap();

    assert_eq!(
        hit.actions,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: false
        }]
    );
    assert_eq!(hit.reasons, vec!["contains word: 'spam'".to_string()]);
    assert_eq!(openai.calls().len(), 1);
}

#[tokio::test]
async fn test_under_not_an_openai_failure_reads_as_a_match() {
    // The accepted price of "failure is no match": negated, it is a match. An
    // owner who puts this condition under a Not gets every message during an
    // outage; nothing in the tree special-cases it.
    let openai = ScriptedAi::answering(Err("OpenAI is down".to_string()));
    let condition = ModerationCondition::Not {
        condition: Box::new(openai_hate("sk-one")),
    };

    let hit = matched_with(&openai, &text_message("hello"), &rule_with(condition)).await;

    assert!(hit.is_some());
}

#[tokio::test]
async fn test_openai_is_asked_once_per_message_however_many_rules_use_the_condition() {
    let openai = ScriptedAi::answering(Ok(OpenAiModerationResult::default()));
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openai_hate("sk-one"),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: ModerationCondition::Any {
                conditions: vec![words("spam"), openai_hate("sk-one")],
            },
        },
    ];

    matched_with(&openai, &text_message("hello"), &rules).await;

    assert_eq!(openai.calls().len(), 1);
}

#[tokio::test]
async fn test_all_does_not_ask_openai_once_an_earlier_condition_fails() {
    // Children are evaluated in the order the owner wrote them and `All` stops
    // at the first miss: that order is how an owner keeps OpenAI off most
    // messages, so it must hold for this condition too.
    let openai = ScriptedAi::answering(Ok(hateful()));
    let condition = ModerationCondition::All {
        conditions: vec![words("crypto"), openai_hate("sk-one")],
    };

    let hit = matched_with(&openai, &text_message("hello"), &rule_with(condition)).await;

    assert!(hit.is_none());
    assert!(openai.calls().is_empty());
}

#[tokio::test]
async fn test_openai_is_not_asked_for_a_rule_whose_actions_are_already_planned() {
    let openai = ScriptedAi::answering(Ok(hateful()));
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: true,
            }],
            condition: words("spam"),
        },
        // A kick that deletes the author's messages covers this; the rule adds
        // nothing, so its condition is never evaluated.
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openai_hate("sk-one"),
        },
    ];

    let hit = matched_with(&openai, &text_message("spam"), &rules).await;

    assert!(hit.is_some());
    assert!(openai.calls().is_empty());
}

// ---------------------------------------------------------------------------
// FlaggedByOpenRouterInstruction
// ---------------------------------------------------------------------------

fn openrouter_instruction(api_key: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOpenRouterInstruction(FlaggedByOpenRouterInstruction {
        retry: Default::default(),
        api_key: api_key.to_string(),
        model: "openai/gpt-4o-mini".to_string(),
        instruction: "Block crypto ads.".to_string(),
        context_messages: 0,
    })
}

#[test]
fn test_openrouter_instruction_wire_format_matches_the_editor_schema() {
    let json = r#"{
        "type": "FlaggedByOpenRouterInstruction",
        "api_key": "sk-or-v1-abc",
        "model": "openai/gpt-4o-mini",
        "instruction": "Block crypto ads."
    }"#;
    let condition: ModerationCondition = serde_json::from_str(json).unwrap();
    assert_eq!(condition, openrouter_instruction("sk-or-v1-abc"));

    let written = serde_json::to_value(&condition).unwrap();
    assert_eq!(
        written,
        serde_json::json!({
            "type": "FlaggedByOpenRouterInstruction",
            "api_key": "sk-or-v1-abc",
            "model": "openai/gpt-4o-mini",
            "instruction": "Block crypto ads.",
            "context_messages": 0,
            "max_attempts": 3,
            "retry_delay_seconds": 1
        })
    );
}

#[test]
fn test_openrouter_instruction_needs_all_three_fields() {
    for json in [
        r#"{ "type": "FlaggedByOpenRouterInstruction", "model": "openai/gpt-4o-mini", "instruction": "i" }"#,
        r#"{ "type": "FlaggedByOpenRouterInstruction", "api_key": "sk", "instruction": "i" }"#,
        r#"{ "type": "FlaggedByOpenRouterInstruction", "api_key": "sk", "model": "openai/gpt-4o-mini" }"#,
    ] {
        assert!(
            serde_json::from_str::<ModerationCondition>(json).is_err(),
            "{json} should not parse"
        );
    }
}

#[tokio::test]
async fn test_a_model_saying_yes_moderates_with_its_reason() {
    let openai = ScriptedAi::judging(Ok(true));

    let hit = matched_with(
        &openai,
        &text_message("buy my coin"),
        &rule_with(openrouter_instruction("sk-one")),
    )
    .await
    .unwrap();

    assert_eq!(hit.actions, vec![ModerationAction::ModerateMessage]);
    assert_eq!(
        hit.reasons,
        vec!["openai/gpt-4o-mini: Promotes a coin.".to_string()]
    );
}

async fn reason_for(reason: &str) -> String {
    let openai = ScriptedAi::judging_with(Ok(true), reason);
    let hit = matched_with(
        &openai,
        &text_message("buy my coin"),
        &rule_with(openrouter_instruction("sk-one")),
    )
    .await
    .unwrap();
    hit.reasons.into_iter().next().unwrap()
}

#[tokio::test]
async fn test_the_models_reason_is_shown_on_one_line() {
    assert_eq!(
        reason_for("  Promotes\na coin\t airdrop.  ").await,
        "openai/gpt-4o-mini: Promotes a coin airdrop."
    );
}

#[tokio::test]
async fn test_a_missing_reason_falls_back_to_saying_whose_verdict_it_is() {
    assert_eq!(
        reason_for(" \n ").await,
        "openai/gpt-4o-mini says it matches the instruction"
    );
}

#[tokio::test]
async fn test_an_overlong_reason_is_cut_to_a_notifications_worth() {
    let reason = reason_for(&"я".repeat(500)).await;
    let shown = reason.strip_prefix("openai/gpt-4o-mini: ").unwrap();
    assert_eq!(shown.chars().count(), 201);
    assert!(shown.ends_with('…'));
}

#[tokio::test]
async fn test_a_model_saying_no_does_not_match() {
    let openai = ScriptedAi::judging(Ok(false));

    let hit = matched_with(
        &openai,
        &text_message("hello"),
        &rule_with(openrouter_instruction("sk-one")),
    )
    .await;

    assert!(hit.is_none());
}

#[tokio::test]
async fn test_the_model_is_asked_with_key_model_instruction_and_the_text_as_is() {
    let openai = ScriptedAi::judging(Ok(false));
    let text = "  Привет,\nмир  ";

    matched_with(
        &openai,
        &text_message(text),
        &rule_with(openrouter_instruction("sk-one")),
    )
    .await;

    assert_eq!(
        openai.instruction_calls(),
        vec![(
            "sk-one".to_string(),
            "openai/gpt-4o-mini".to_string(),
            "Block crypto ads.".to_string(),
            text.to_string()
        )]
    );
    assert!(
        openai.calls().is_empty(),
        "the moderation model was asked too"
    );
}

#[tokio::test]
async fn test_the_model_is_not_asked_about_a_message_without_text() {
    let openai = ScriptedAi::judging(Ok(true));
    let rules = rule_with(openrouter_instruction("sk-one"));
    let captionless_picture = GroupMessage {
        attachment: Some(MessageAttachment::Image),
        ..Default::default()
    };

    assert!(
        matched_with(&openai, &text_message("  \n "), &rules)
            .await
            .is_none()
    );
    assert!(
        matched_with(&openai, &captionless_picture, &rules)
            .await
            .is_none()
    );
    assert!(openai.instruction_calls().is_empty());
}

#[tokio::test]
async fn test_no_verdict_reads_as_no_match_and_other_rules_still_apply() {
    let openai = ScriptedAi::judging(Err("OpenRouter is down".to_string()));
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openrouter_instruction("sk-one"),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: words("spam"),
        },
    ];

    let hit = matched_with(&openai, &text_message("spam"), &rules)
        .await
        .unwrap();

    assert_eq!(hit.reasons, vec!["contains word: 'spam'".to_string()]);
}

#[tokio::test]
async fn test_the_model_is_asked_once_per_message_however_many_rules_use_the_condition() {
    let openai = ScriptedAi::judging(Ok(false));
    let rules = vec![
        ModerationRule {
            actions: vec![ModerationAction::ModerateMessage],
            condition: openrouter_instruction("sk-one"),
        },
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: ModerationCondition::Any {
                conditions: vec![words("spam"), openrouter_instruction("sk-one")],
            },
        },
    ];

    matched_with(&openai, &text_message("hello"), &rules).await;

    assert_eq!(openai.instruction_calls().len(), 1);
}

#[tokio::test]
async fn test_all_does_not_ask_the_model_once_an_earlier_condition_fails() {
    let openai = ScriptedAi::judging(Ok(true));
    let condition = ModerationCondition::All {
        conditions: vec![words("crypto"), openrouter_instruction("sk-one")],
    };

    let hit = matched_with(&openai, &text_message("hello"), &rule_with(condition)).await;

    assert!(hit.is_none());
    assert!(openai.instruction_calls().is_empty());
}

// ---------------------------------------------------------------------------
// FlaggedByOpenRouterInstruction: earlier messages as context
// ---------------------------------------------------------------------------

fn instruction_with_context(context_messages: u32) -> ModerationCondition {
    let mut condition = openrouter_instruction("sk-one");
    if let ModerationCondition::FlaggedByOpenRouterInstruction(FlaggedByOpenRouterInstruction {
        context_messages: n,
        ..
    }) = &mut condition
    {
        *n = context_messages;
    }
    condition
}

async fn judged_with_history(
    openai: &ScriptedAi,
    history: &dyn crate::domain::moderator::ports::GroupMessageHistoryRepository,
    msg: &GroupMessage,
    condition: ModerationCondition,
) -> Option<ModerationMatch> {
    should_moderate(
        msg,
        &rule_with(condition),
        &ConditionPorts {
            activity_repo: &InMemoryUserMessageActivityRepository::new(),
            character_activity_repo: &no_character_activity(),
            line_activity_repo: &no_line_activity(),
            moderation_activity_repo: &InMemoryUserModerationActivityRepository::new(),
            group_activity_repo: &no_group_activity(),
            group_character_activity_repo: &no_group_character_activity(),
            group_line_activity_repo: &no_group_line_activity(),
            message_history: history,
            openai: openai,
            openrouter: openai,
        },
    )
    .await
    .unwrap()
}

async fn history_of(messages: &[(i64, UserId, &str)]) -> InMemoryGroupMessageHistoryRepository {
    use crate::domain::moderator::ports::{GroupMessageHistoryRepository, RecentGroupMessage};
    let history = no_message_history();
    for (message_id, author_id, text) in messages {
        history
            .record_message(
                &1,
                RecentGroupMessage {
                    message_id: *message_id,
                    author_id: *author_id,
                    author_name: format!("name {author_id}"),
                    text: text.to_string(),
                    attachment: None,
                    timestamp: Utc::now(),
                },
                10,
            )
            .await
            .unwrap();
    }
    history
}

fn message_in_group_1(message_id: i64, author_id: UserId, text: &str) -> GroupMessage {
    let mut msg = text_message(text);
    msg.group.id = 1;
    msg.message_id = message_id;
    msg.author_id = author_id;
    msg.author_name = format!("name {author_id}");
    msg.timestamp = Utc::now();
    msg
}

#[tokio::test]
async fn test_the_latest_earlier_messages_go_along_with_their_authors_names() {
    use crate::domain::moderator::ports::InstructionContextMessage;
    let openai = ScriptedAi::judging(Ok(false));
    let history = history_of(&[
        (1, 50, "too old"),
        (2, 60, "anyone selling?"),
        (3, 70, "yes"),
    ])
    .await;

    judged_with_history(
        &openai,
        &history,
        &message_in_group_1(4, 60, "me, dm"),
        instruction_with_context(2),
    )
    .await;

    assert_eq!(
        openai.contexts(),
        vec![vec![
            InstructionContextMessage {
                author_name: "name 60".to_string(),
                text: "anyone selling?".to_string(),
                attachment: None,
            },
            InstructionContextMessage {
                author_name: "name 70".to_string(),
                text: "yes".to_string(),
                attachment: None,
            },
        ]]
    );
    assert_eq!(openai.judged_authors(), vec!["name 60".to_string()]);
}

#[tokio::test]
async fn test_no_earlier_message_goes_along_unless_asked() {
    let openai = ScriptedAi::judging(Ok(false));
    let history = history_of(&[(1, 50, "hello")]).await;

    judged_with_history(
        &openai,
        &history,
        &message_in_group_1(2, 60, "hi"),
        instruction_with_context(0),
    )
    .await;

    assert_eq!(openai.contexts(), vec![Vec::new()]);
}

/// The history cannot be read.
struct BrokenHistory;

#[async_trait::async_trait]
impl crate::domain::moderator::ports::GroupMessageHistoryRepository for BrokenHistory {
    async fn record_message(
        &self,
        _group_id: &MessengerGroupId,
        _message: crate::domain::moderator::ports::RecentGroupMessage,
        _keep: u32,
    ) -> Result<(), Err> {
        Err("broken".into())
    }

    async fn record_edit(
        &self,
        _group_id: &MessengerGroupId,
        _message: crate::domain::moderator::ports::RecentGroupMessage,
    ) -> Result<(), Err> {
        Err("broken".into())
    }

    async fn forget_message(
        &self,
        _group_id: &MessengerGroupId,
        _message_id: &crate::domain::moderator::ports::MessageId,
    ) -> Result<(), Err> {
        Err("broken".into())
    }

    async fn messages_before(
        &self,
        _group_id: &MessengerGroupId,
        _message_id: &crate::domain::moderator::ports::MessageId,
        _count: u32,
        _now: DateTime<Utc>,
    ) -> Result<Vec<crate::domain::moderator::ports::RecentGroupMessage>, Err> {
        Err("broken".into())
    }
}

#[tokio::test]
async fn test_a_history_that_cannot_be_read_is_no_verdict_and_asks_nobody() {
    let openai = ScriptedAi::judging(Ok(true));

    let hit = judged_with_history(
        &openai,
        &BrokenHistory,
        &message_in_group_1(2, 60, "buy my coin"),
        instruction_with_context(3),
    )
    .await;

    assert!(hit.is_none());
    assert!(openai.instruction_calls().is_empty());
}

#[tokio::test]
async fn test_length_conditions_see_the_untrimmed_message() {
    use crate::domain::moderator::ports::GroupMessage;
    use crate::domain::moderator::ports::conditions::{
        ExceedsMaxCharacters, ExceedsMaxLines, ExceedsMaxWords, IsBlank,
    };
    use crate::domain::moderator::rules::{
        ModerationAction, ModerationCondition, ModerationRule,
        should_moderate as top_level_moderate,
    };
    use crate::infrastructure::adapters::group_character_activity_repo_in_memory::InMemoryGroupCharacterActivityRepository;
    use crate::infrastructure::adapters::group_line_activity_repo_in_memory::InMemoryGroupLineActivityRepository;
    use crate::infrastructure::adapters::group_message_activity_repo_in_memory::InMemoryGroupMessageActivityRepository;
    use crate::infrastructure::adapters::group_message_history_repo_in_memory::InMemoryGroupMessageHistoryRepository;
    use crate::infrastructure::adapters::user_character_activity_repo_in_memory::InMemoryUserCharacterActivityRepository;
    use crate::infrastructure::adapters::user_line_activity_repo_in_memory::InMemoryUserLineActivityRepository;
    use crate::infrastructure::adapters::user_message_activity_repo_in_memory::InMemoryUserMessageActivityRepository;
    use crate::infrastructure::adapters::user_moderation_activity_repo_in_memory::InMemoryUserModerationActivityRepository;

    let repo = InMemoryUserMessageActivityRepository::new();
    let char_repo = InMemoryUserCharacterActivityRepository::new();
    let line_repo = InMemoryUserLineActivityRepository::new();
    let mod_repo = InMemoryUserModerationActivityRepository::new();
    let group_repo = InMemoryGroupMessageActivityRepository::new();
    let group_char_repo = InMemoryGroupCharacterActivityRepository::new();
    let group_line_repo = InMemoryGroupLineActivityRepository::new();
    let history = InMemoryGroupMessageHistoryRepository::new();
    let msg = |text: &str| GroupMessage {
        text: text.to_string(),
        ..Default::default()
    };

    let rules = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::Any {
            conditions: vec![
                ModerationCondition::IsBlank(IsBlank {}),
                ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters {
                    max_characters: 100,
                }),
                ModerationCondition::ExceedsMaxWords(ExceedsMaxWords { max_words: 10 }),
                ModerationCondition::ExceedsMaxLines(ExceedsMaxLines {
                    max_lines: 5,
                    chars_per_line: 40,
                }),
            ],
        },
    }];

    let matches = async |text: &str, rules: &[ModerationRule]| {
        top_level_moderate(
            &msg(text),
            rules,
            &ConditionPorts {
                activity_repo: &repo,
                character_activity_repo: &char_repo,
                line_activity_repo: &line_repo,
                moderation_activity_repo: &mod_repo,
                group_activity_repo: &group_repo,
                group_character_activity_repo: &group_char_repo,
                group_line_activity_repo: &group_line_repo,
                message_history: &history,
                openai: &UnusedAi,
                openrouter: &UnusedAi,
            },
        )
        .await
        .unwrap()
        .is_some()
    };

    // Normal message passes
    assert!(!matches("Hello world", &rules).await);

    // Leading and trailing newlines are NOT stripped by a top-level trim
    assert!(matches(&format!(".{}", "\n".repeat(500)), &rules).await);
    assert!(matches(&format!("{}.", "\n".repeat(500)), &rules).await);

    // 500 trailing spaces exceed max_characters
    assert!(matches(&format!(".{}", " ".repeat(500)), &rules).await);

    // A blank message matches through IsBlank
    assert!(matches("   ", &rules).await);

    // Without IsBlank, a short blank message matches nothing
    let length_only = vec![ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition: ModerationCondition::ExceedsMaxCharacters(ExceedsMaxCharacters {
            max_characters: 100,
        }),
    }];
    assert!(!matches("   ", &length_only).await);
}
