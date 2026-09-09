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
            condition: RuleCondition::WordsBlacklist {
                keywords: vec!["word1".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::TriggeredMessage,
            },
            condition: RuleCondition::MessagesBlacklist {
                messages: vec!["msg1".to_string()],
                case_sensitive: true,
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::None,
            },
            condition: RuleCondition::LinksBlacklist {
                blocked: vec!["spam.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::ModerateMessage,
            condition: RuleCondition::LinksWhitelist {
                allowed: vec!["ok.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages,
            },
            condition: RuleCondition::LinksWhitelistTop100 {
                allowed: vec!["extra.com".to_string()],
            },
        },
        ModerationRule {
            action: ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage,
            },
            condition: RuleCondition::ScreenFlooding {
                max_characters: 100,
                max_words: 20,
                max_lines: 5,
                chars_per_line: 40,
                disallow_empty_messages: true,
                disallow_invisible_chars: true,
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
        assert_eq!(action_count, 6);
    }

    // Delete group data
    repo.delete_group_data(&messenger_group_id).await.unwrap();

    // Verify all tables are completely empty
    {
        let guard = conn.lock().unwrap();
        let tables = [
            "moderation_groups",
            "moderation_rule__words_blacklist",
            "moderation_rule__words_blacklist__keywords",
            "moderation_rule__messages_blacklist",
            "moderation_rule__messages_blacklist__messages",
            "moderation_rule__links_blacklist",
            "moderation_rule__links_blacklist__domains",
            "moderation_rule__links_whitelist",
            "moderation_rule__links_whitelist__domains",
            "moderation_rule__links_whitelist_top100",
            "moderation_rule__links_whitelist_top100__allowed",
            "moderation_rule__screen_flooding",
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
