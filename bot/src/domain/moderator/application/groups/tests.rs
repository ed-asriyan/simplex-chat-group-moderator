//! Rule saving: conditions are validated before they reach the repository.

use super::GroupAdministrationApplication;
use crate::domain::moderator::application::tests::{MockGroupModerator, MockModerationRepository};
use crate::domain::moderator::message_filter::ModerationCondition;
use crate::domain::moderator::ports::{
    Group, GroupAdministration, GroupId, ModerationAction, ModerationRule, UserId,
};
use std::sync::Arc;

fn app_owning_group(group_id: GroupId, owner_id: UserId) -> GroupAdministrationApplication {
    GroupAdministrationApplication::new(
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
                actions: vec![ModerationAction::ModerateMessage],
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
            actions: vec![ModerationAction::ModerateMessage],
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
                actions: vec![ModerationAction::ModerateMessage],
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

#[tokio::test]
async fn test_set_group_rules_rejects_too_long_observer_duration() {
    let app = app_owning_group(10, 100);

    let err = app
        .set_group_rules(
            100,
            10,
            vec![ModerationRule {
                actions: vec![ModerationAction::SetAuthorObserver {
                    duration_minutes: 43_201,
                }],
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["badword".to_string()],
                },
            }],
        )
        .await
        .expect_err("an observer restriction longer than a month should be rejected");

    assert!(
        err.to_string().contains("Observer duration too long"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_set_group_rules_accepts_observer_durations_up_to_a_month() {
    let app = app_owning_group(10, 100);

    for duration_minutes in [0, 1, 43_200] {
        app.set_group_rules(
            100,
            10,
            vec![ModerationRule {
                actions: vec![ModerationAction::SetAuthorObserver { duration_minutes }],
                condition: ModerationCondition::ContainsWords {
                    keywords: vec!["badword".to_string()],
                },
            }],
        )
        .await
        .unwrap();
    }
}
