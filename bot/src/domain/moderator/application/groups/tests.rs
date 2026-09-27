//! Rule saving: conditions are validated, and OpenAI keys checked with OpenAI,
//! before anything reaches the repository.

use super::GroupAdministrationApplication;
use crate::domain::moderator::application::tests::{MockGroupModerator, MockModerationRepository};
use crate::domain::moderator::message_filter::ModerationCondition;
use crate::domain::moderator::ports::{
    CategoryTrigger, Err, Group, GroupAdministration, GroupId, KeyCheck, MessengerGroupId,
    ModerationAction, ModerationRepository, ModerationRule, OpenAi, OpenAiCategoryTriggers,
    OpenAiInstructionVerdict, OpenAiModerationResult, OwnedModerationRule, UserId,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The verifier for rule sets without an OpenAI key: being asked is the bug.
struct UnusedKeyVerifier;

#[async_trait]
impl OpenAi for UnusedKeyVerifier {
    async fn classify(&self, _api_key: &str, _text: &str) -> Result<OpenAiModerationResult, Err> {
        panic!("no message is moderated while saving rules")
    }

    async fn matches_instruction(
        &self,
        _api_key: &str,
        _model: &str,
        _instruction: &str,
        _text: &str,
    ) -> Result<OpenAiInstructionVerdict, Err> {
        panic!("no message is moderated while saving rules")
    }

    async fn verify(&self, _api_key: &str) -> KeyCheck {
        panic!("OpenAI was asked about a key although no rule carries one")
    }

    async fn verify_model(&self, _api_key: &str, _model: &str) -> KeyCheck {
        panic!("OpenAI was asked about a key although no rule carries one")
    }
}

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
        Arc::new(UnusedKeyVerifier),
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

// ---------------------------------------------------------------------------
// OpenAI keys
// ---------------------------------------------------------------------------

/// Answers `Valid` unless told otherwise, and records every key it was asked about.
#[derive(Default)]
struct FakeKeyVerifier {
    answers: HashMap<String, KeyCheck>,
    asked: Mutex<Vec<String>>,
}

impl FakeKeyVerifier {
    fn answering(answers: &[(&str, KeyCheck)]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers
                .iter()
                .map(|(key, check)| (key.to_string(), *check))
                .collect(),
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<String> {
        let mut asked = self.asked.lock().unwrap().clone();
        asked.sort();
        asked
    }
}

#[async_trait]
impl OpenAi for FakeKeyVerifier {
    async fn classify(&self, _api_key: &str, _text: &str) -> Result<OpenAiModerationResult, Err> {
        panic!("no message is moderated while saving rules")
    }

    async fn matches_instruction(
        &self,
        _api_key: &str,
        _model: &str,
        _instruction: &str,
        _text: &str,
    ) -> Result<OpenAiInstructionVerdict, Err> {
        panic!("no message is moderated while saving rules")
    }

    async fn verify(&self, api_key: &str) -> KeyCheck {
        self.asked.lock().unwrap().push(api_key.to_string());
        self.answers
            .get(api_key)
            .copied()
            .unwrap_or(KeyCheck::Valid)
    }

    /// Recorded, and answered, as `key@model`.
    async fn verify_model(&self, api_key: &str, model: &str) -> KeyCheck {
        let asked = format!("{api_key}@{model}");
        self.asked.lock().unwrap().push(asked.clone());
        self.answers.get(&asked).copied().unwrap_or(KeyCheck::Valid)
    }
}

/// Owns group 10 for user 100 and keeps whatever rules were last saved.
#[derive(Default)]
struct SavingRepository {
    saved: Mutex<Option<Vec<ModerationRule>>>,
}

impl SavingRepository {
    fn saved(&self) -> Option<Vec<ModerationRule>> {
        self.saved.lock().unwrap().clone()
    }
}

#[async_trait]
impl ModerationRepository for SavingRepository {
    async fn save_owner(&self, _: &MessengerGroupId, _: &str, _: &UserId) -> Result<GroupId, Err> {
        Ok(10)
    }
    async fn get_owner_by_messenger_id(&self, _: &MessengerGroupId) -> Result<Option<UserId>, Err> {
        Ok(Some(100))
    }
    async fn get_groups_by_owner_id(&self, _: &UserId) -> Result<Vec<Group>, Err> {
        Ok(vec![])
    }
    async fn get_owner_by_id(&self, _: &GroupId) -> Result<Option<UserId>, Err> {
        Ok(Some(100))
    }
    async fn set_group_name(&self, _: &MessengerGroupId, _: &str) -> Result<(), Err> {
        Ok(())
    }
    async fn get_group_rules(&self, _: &GroupId) -> Result<Vec<OwnedModerationRule>, Err> {
        Ok(vec![])
    }
    async fn get_group_rules_by_messenger_id(
        &self,
        _: &MessengerGroupId,
    ) -> Result<Vec<OwnedModerationRule>, Err> {
        Ok(vec![])
    }
    async fn set_group_rules(&self, _: &GroupId, rules: &[ModerationRule]) -> Result<(), Err> {
        *self.saved.lock().unwrap() = Some(rules.to_vec());
        Ok(())
    }
    async fn delete_group_data(&self, _: &MessengerGroupId) -> Result<(), Err> {
        Ok(())
    }
    async fn get_group_by_messenger_id(&self, _: &MessengerGroupId) -> Result<Option<Group>, Err> {
        Ok(None)
    }
    async fn set_notifications_enabled(&self, _: &GroupId, _: bool) -> Result<(), Err> {
        Ok(())
    }
    async fn set_dry_mode_enabled(&self, _: &GroupId, _: bool) -> Result<(), Err> {
        Ok(())
    }
}

fn app_with(
    repository: &Arc<SavingRepository>,
    verifier: &Arc<FakeKeyVerifier>,
) -> GroupAdministrationApplication {
    GroupAdministrationApplication::new(
        repository.clone(),
        Arc::new(MockGroupModerator::default()),
        verifier.clone(),
    )
}

fn openai_condition(api_key: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOmniModeration {
        api_key: api_key.to_string(),
        triggers: OpenAiCategoryTriggers {
            hate: CategoryTrigger::OpenAiDecides,
            ..Default::default()
        },
    }
}

fn moderate_when(condition: ModerationCondition) -> ModerationRule {
    ModerationRule {
        actions: vec![ModerationAction::ModerateMessage],
        condition,
    }
}

#[tokio::test]
async fn test_a_key_openai_accepts_is_saved() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);
    let rules = vec![moderate_when(openai_condition("sk-good"))];

    app_with(&repository, &verifier)
        .set_group_rules(100, 10, rules.clone())
        .await
        .unwrap();

    assert_eq!(verifier.asked(), vec!["sk-good"]);
    assert_eq!(repository.saved(), Some(rules));
}

#[tokio::test]
async fn test_a_key_openai_does_not_accept_is_not_saved_and_the_owner_is_told_why() {
    let cases = [
        (KeyCheck::Rejected, "rejected"),
        (KeyCheck::Forbidden, "Moderations"),
        (KeyCheck::QuotaExceeded, "quota"),
        (KeyCheck::Unreachable, "reach OpenAI"),
    ];
    let key = "sk-proj-secret-part-WXYZ";
    for (check, says) in cases {
        let repository = Arc::new(SavingRepository::default());
        let verifier = FakeKeyVerifier::answering(&[(key, check)]);

        let err = app_with(&repository, &verifier)
            .set_group_rules(100, 10, vec![moderate_when(openai_condition(key))])
            .await
            .expect_err("the rules must not be saved")
            .to_string();

        assert!(err.contains(says), "{check:?}: unexpected error: {err}");
        // The owner can tell which key it was by its last characters; the
        // error, which lands in the chat and the logs, never carries the key.
        assert!(err.contains("WXYZ"), "{check:?}: no hint of the key: {err}");
        assert!(
            !err.contains(key),
            "{check:?}: the error leaks the key: {err}"
        );
        assert_eq!(repository.saved(), None, "{check:?}");
    }
}

#[tokio::test]
async fn test_every_distinct_key_is_asked_about_once_wherever_it_sits() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);
    let rules = vec![
        moderate_when(openai_condition("sk-one")),
        moderate_when(ModerationCondition::All {
            conditions: vec![
                ModerationCondition::ContainsWords {
                    keywords: vec!["crypto".to_string()],
                },
                ModerationCondition::Not {
                    condition: Box::new(openai_condition("sk-two")),
                },
            ],
        }),
        ModerationRule {
            actions: vec![ModerationAction::KickAuthor {
                delete_all_messages: false,
            }],
            condition: openai_condition("sk-one"),
        },
    ];

    app_with(&repository, &verifier)
        .set_group_rules(100, 10, rules)
        .await
        .unwrap();

    assert_eq!(verifier.asked(), vec!["sk-one", "sk-two"]);
}

#[tokio::test]
async fn test_one_bad_key_keeps_the_whole_rule_set_from_being_saved() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[("sk-bad", KeyCheck::Rejected)]);
    let rules = vec![
        moderate_when(openai_condition("sk-good")),
        moderate_when(openai_condition("sk-bad")),
    ];

    assert!(
        app_with(&repository, &verifier)
            .set_group_rules(100, 10, rules)
            .await
            .is_err()
    );
    assert_eq!(repository.saved(), None);
}

#[tokio::test]
async fn test_the_key_is_checked_as_it_will_be_stored() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);

    app_with(&repository, &verifier)
        .set_group_rules(
            100,
            10,
            vec![moderate_when(openai_condition("  sk-good \n"))],
        )
        .await
        .unwrap();

    assert_eq!(verifier.asked(), vec!["sk-good"]);
    assert_eq!(
        repository.saved(),
        Some(vec![moderate_when(openai_condition("sk-good"))])
    );
}

#[tokio::test]
async fn test_a_malformed_condition_is_rejected_before_openai_is_asked() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);
    let every_category_off = ModerationCondition::FlaggedByOmniModeration {
        api_key: "sk-good".to_string(),
        triggers: OpenAiCategoryTriggers::default(),
    };

    let err = app_with(&repository, &verifier)
        .set_group_rules(100, 10, vec![moderate_when(every_category_off)])
        .await
        .expect_err("a condition that can never match is rejected")
        .to_string();

    assert!(
        err.contains("every category off"),
        "unexpected error: {err}"
    );
    assert!(verifier.asked().is_empty());
    assert_eq!(repository.saved(), None);
}

#[tokio::test]
async fn test_rules_without_a_key_never_ask_openai() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);

    app_with(&repository, &verifier)
        .set_group_rules(
            100,
            10,
            vec![moderate_when(ModerationCondition::ContainsWords {
                keywords: vec!["spam".to_string()],
            })],
        )
        .await
        .unwrap();

    assert!(verifier.asked().is_empty());
    assert!(repository.saved().is_some());
}

#[tokio::test]
async fn test_a_non_owner_never_gets_openai_asked_about_a_key() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);

    assert!(
        app_with(&repository, &verifier)
            .set_group_rules(999, 10, vec![moderate_when(openai_condition("sk-good"))])
            .await
            .is_err()
    );
    assert!(verifier.asked().is_empty());
}

fn instructed(api_key: &str, model: &str) -> ModerationCondition {
    ModerationCondition::FlaggedByOpenAiInstruction {
        api_key: api_key.to_string(),
        model: model.to_string(),
        instruction: "Block crypto ads.".to_string(),
    }
}

#[tokio::test]
async fn test_a_model_key_is_checked_with_its_model() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);
    let rules = vec![moderate_when(instructed("sk-good", "gpt-4o-mini"))];

    app_with(&repository, &verifier)
        .set_group_rules(100, 10, rules.clone())
        .await
        .unwrap();

    assert_eq!(verifier.asked(), vec!["sk-good@gpt-4o-mini"]);
    assert_eq!(repository.saved(), Some(rules));
}

#[tokio::test]
async fn test_each_distinct_key_and_model_pair_is_checked_once() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);
    let rules = vec![
        moderate_when(instructed("sk-one", "gpt-4o-mini")),
        moderate_when(ModerationCondition::Not {
            condition: Box::new(instructed("sk-one", "gpt-4o-mini")),
        }),
        moderate_when(instructed("sk-one", "gpt-4.1-mini")),
        // The same key used for moderation is a separate check: another
        // endpoint, another permission.
        moderate_when(openai_condition("sk-one")),
    ];

    app_with(&repository, &verifier)
        .set_group_rules(100, 10, rules)
        .await
        .unwrap();

    assert_eq!(
        verifier.asked(),
        vec!["sk-one", "sk-one@gpt-4.1-mini", "sk-one@gpt-4o-mini"]
    );
}

#[tokio::test]
async fn test_a_model_the_key_cannot_use_is_not_saved_and_the_owner_is_told_which() {
    let key = "sk-proj-secret-part-WXYZ";
    let cases = [
        (KeyCheck::ModelUnavailable, "cannot use the model gpt-4.1"),
        (KeyCheck::Forbidden, "Responses"),
        (KeyCheck::Rejected, "rejected"),
        (KeyCheck::QuotaExceeded, "quota"),
        (KeyCheck::Unreachable, "reach OpenAI"),
    ];
    for (check, says) in cases {
        let repository = Arc::new(SavingRepository::default());
        let verifier = FakeKeyVerifier::answering(&[(&format!("{key}@gpt-4.1"), check)]);

        let err = app_with(&repository, &verifier)
            .set_group_rules(100, 10, vec![moderate_when(instructed(key, "gpt-4.1"))])
            .await
            .expect_err("the rules must not be saved")
            .to_string();

        assert!(err.contains(says), "{check:?}: unexpected error: {err}");
        assert!(
            err.contains("WXYZ") && !err.contains(key),
            "{check:?}: {err}"
        );
        assert_eq!(repository.saved(), None, "{check:?}");
    }
}

#[tokio::test]
async fn test_an_unlisted_model_is_rejected_before_openai_is_asked() {
    let repository = Arc::new(SavingRepository::default());
    let verifier = FakeKeyVerifier::answering(&[]);

    let err = app_with(&repository, &verifier)
        .set_group_rules(100, 10, vec![moderate_when(instructed("sk-good", "gpt-5"))])
        .await
        .expect_err("an unlisted model is rejected")
        .to_string();

    assert!(err.contains("cannot use the model 'gpt-5'"), "{err}");
    assert!(verifier.asked().is_empty());
}
