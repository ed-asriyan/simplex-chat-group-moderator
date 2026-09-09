use super::*;
use crate::infrastructure::migrations;

#[tokio::test]
async fn test_delete_group_data_removes_all_conditions_and_actions() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let messenger_group_id = 999;
    let owner_id = 123;
    let group_id = repo
        .save_owner(&messenger_group_id, "Test Group", &owner_id)
        .await
        .unwrap();

    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: RuleCondition::ContainsBannedWords {
                keywords: vec!["word1".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: RuleCondition::MatchesExactMessage {
                messages: vec!["msg1".to_string()],
                case_sensitive: true,
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
            condition: RuleCondition::ContainsLinksToForbiddenWebsites {
                blocked: vec!["spam.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: RuleCondition::ContainsLinksOutsideAllowedList {
                allowed: vec!["ok.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: RuleCondition::ContainsLinksOutsideTop100 {
                allowed: vec!["extra.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: RuleCondition::FloodsChatOrExceedsLimits {
                max_characters: 100,
                max_words: 20,
                max_lines: 5,
                chars_per_line: 40,
                disallow_empty_messages: true,
                disallow_invisible_chars: true,
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: RuleCondition::UserExceedsMessagesRateLimit {
                message_count: 5,
                time_window_minutes: 2,
            },
        },
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    // Verify data exists before deletion
    {
        let guard = conn.lock().unwrap();
        let action_count: i64 = guard
            .query_row("SELECT COUNT(*) FROM moderation_actions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(action_count, 7);
    }

    // Delete group data
    repo.delete_group_data(&messenger_group_id).await.unwrap();

    // Verify all tables are completely empty
    {
        let guard = conn.lock().unwrap();
        let tables = [
            "moderation_groups",
            "moderation_rule__contains_banned_words",
            "moderation_rule__contains_banned_words__keywords",
            "moderation_rule__matches_exact_message",
            "moderation_rule__matches_exact_message__messages",
            "moderation_rule__contains_links_to_forbidden_websites",
            "moderation_rule__contains_links_to_forbidden_websites__domains",
            "moderation_rule__contains_links_outside_allowed_list",
            "moderation_rule__contains_links_outside_allowed_list__domains",
            "moderation_rule__contains_links_outside_top100",
            "moderation_rule__contains_links_outside_top100__allowed",
            "moderation_rule__floods_chat_or_exceeds_limits",
            "moderation_rule__user_exceeds_messages_rate_limit",
            "moderation_actions",
            "moderation_action__moderate_message",
            "moderation_action__kick_author",
            "moderation_action__set_author_observer",
        ];
        for table in tables {
            let count: i64 = guard
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0, "Table {table} should be empty after delete_group_data");
        }
    }
}

#[tokio::test]
async fn test_save_and_load_rate_limit_rules() {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();

    let repo = SqliteModerationRepository::new(conn.clone());
    let messenger_group_id = 1001;
    let owner_id = 456;
    let group_id = repo
        .save_owner(&messenger_group_id, "Rate Limit Test Group", &owner_id)
        .await
        .unwrap();

    let rules = vec![
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: RuleCondition::UserExceedsMessagesRateLimit {
                message_count: 3,
                time_window_minutes: 1,
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: RuleCondition::UserExceedsMessagesRateLimit {
                message_count: 10,
                time_window_minutes: 60,
            },
        },
    ];

    repo.set_group_rules(&group_id, &rules).await.unwrap();

    let loaded = repo.get_group_rules(&group_id).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].rule, rules[0]);
    assert_eq!(loaded[1].rule, rules[1]);
}

