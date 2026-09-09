use super::*;

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
        RuleCondition::ContainsBannedWords { keywords } => {
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
        condition: RuleCondition::MatchesExactMessage {
            messages: vec!["banned message".to_string()],
            case_sensitive: false,
        },
    };

    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: ModerationRule = serde_json::from_str(&serialized).unwrap();
    assert_eq!(rule, deserialized);
}

#[test]
fn test_should_moderate_returns_matching_action_and_reason() {
    let rules = vec![ModerationRule {
        action: ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
        condition: RuleCondition::ContainsBannedWords {
            keywords: vec!["danger".to_string()],
        },
    }];

    let result = should_moderate("This is a danger message", &rules);
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(
        m.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        }
    );
    assert_eq!(m.reason, "blacklisted word: 'danger'");
}

#[test]
fn test_rules_applied_in_order_first_match_wins() {
    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: RuleCondition::ContainsBannedWords {
                keywords: vec!["first".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: RuleCondition::ContainsBannedWords {
                keywords: vec!["second".to_string()],
            },
        },
    ];

    // Both keywords present; first rule must win
    let result = should_moderate("first and second both match", &rules);
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.action, ModerationAction::ModerateMessage);
    assert_eq!(m.reason, "blacklisted word: 'first'");

    // Reversed rules list: now KickAuthor must win
    let reversed_rules = vec![rules[1].clone(), rules[0].clone()];
    let reversed_result = should_moderate("first and second both match", &reversed_rules);
    assert!(reversed_result.is_some());
    let rm = reversed_result.unwrap();
    assert_eq!(
        rm.action,
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        }
    );
    assert_eq!(rm.reason, "blacklisted word: 'second'");
}

#[test]
fn test_no_rules_match_returns_none() {
    let rules = vec![ModerationRule {
        action: ModerationAction::ModerateMessage,
        condition: RuleCondition::ContainsBannedWords {
            keywords: vec!["banned".to_string()],
        },
    }];

    assert!(should_moderate("all good here", &rules).is_none());
}
