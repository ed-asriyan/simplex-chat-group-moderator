use super::should_moderate;
use crate::domain::moderator::ports::{
    CategoryTrigger, OpenAiCategory, OpenAiCategoryTriggers, OpenAiModerationResult,
};

fn verdict(flagged: &[OpenAiCategory], scores: &[(OpenAiCategory, f64)]) -> OpenAiModerationResult {
    OpenAiModerationResult {
        flagged: flagged.iter().copied().collect(),
        scores: scores.iter().copied().collect(),
    }
}

fn only(category: OpenAiCategory, trigger: CategoryTrigger) -> OpenAiCategoryTriggers {
    let mut triggers = OpenAiCategoryTriggers::default();
    triggers.set(category, trigger);
    triggers
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

#[test]
fn test_every_category_off_never_matches() {
    let everything: Vec<(OpenAiCategory, f64)> =
        OpenAiCategory::ALL.iter().map(|c| (*c, 1.0)).collect();
    let v = verdict(&OpenAiCategory::ALL, &everything);
    assert_eq!(
        should_moderate(&OpenAiCategoryTriggers::default(), &v),
        None
    );
}

#[test]
fn test_openai_decides_matches_what_openai_flags() {
    let triggers = only(OpenAiCategory::Hate, CategoryTrigger::OpenAiDecides);
    let v = verdict(&[OpenAiCategory::Hate], &[(OpenAiCategory::Hate, 0.4)]);
    assert_eq!(
        should_moderate(&triggers, &v),
        Some("flagged by OpenAI Omni: hate (OpenAI)".to_string())
    );
}

#[test]
fn test_openai_decides_ignores_a_high_score_openai_did_not_flag() {
    // OpenAI's threshold is its own business: a high score it did not flag is
    // not a match under "OpenAI decides".
    let triggers = only(OpenAiCategory::Hate, CategoryTrigger::OpenAiDecides);
    let v = verdict(&[], &[(OpenAiCategory::Hate, 0.99)]);
    assert_eq!(should_moderate(&triggers, &v), None);
}

#[test]
fn test_min_score_is_inclusive() {
    let triggers = only(
        OpenAiCategory::Violence,
        CategoryTrigger::MinScorePercent(80),
    );
    assert_eq!(
        should_moderate(
            &triggers,
            &verdict(&[], &[(OpenAiCategory::Violence, 0.80)])
        ),
        Some("flagged by OpenAI Omni: violence 80% ≥ 80%".to_string())
    );
    assert_eq!(
        should_moderate(
            &triggers,
            &verdict(&[], &[(OpenAiCategory::Violence, 0.79)])
        ),
        None
    );
}

#[test]
fn test_min_score_is_exact_at_every_percentage() {
    // 0.29 * 100.0 is 28.999999999999996 in f64. A plain comparison would miss
    // a message the owner asked to delete at exactly the score they typed.
    for percent in 1..=100u8 {
        let triggers = only(
            OpenAiCategory::Hate,
            CategoryTrigger::MinScorePercent(percent),
        );
        let at = f64::from(percent) / 100.0;
        assert!(
            should_moderate(&triggers, &verdict(&[], &[(OpenAiCategory::Hate, at)])).is_some(),
            "score {at} should match a threshold of {percent}%"
        );
        let below = f64::from(percent - 1) / 100.0;
        assert!(
            should_moderate(&triggers, &verdict(&[], &[(OpenAiCategory::Hate, below)])).is_none(),
            "score {below} should not match a threshold of {percent}%"
        );
    }
}

#[test]
fn test_min_score_can_be_more_lenient_than_openai() {
    // The reason the trigger is per category: a gaming group sets violence to
    // 95% and OpenAI's own, stricter flag must not override it.
    let triggers = only(
        OpenAiCategory::Violence,
        CategoryTrigger::MinScorePercent(95),
    );
    let v = verdict(
        &[OpenAiCategory::Violence],
        &[(OpenAiCategory::Violence, 0.60)],
    );
    assert_eq!(should_moderate(&triggers, &v), None);
}

#[test]
fn test_min_score_can_be_stricter_than_openai() {
    let triggers = only(
        OpenAiCategory::Harassment,
        CategoryTrigger::MinScorePercent(30),
    );
    let v = verdict(&[], &[(OpenAiCategory::Harassment, 0.35)]);
    assert_eq!(
        should_moderate(&triggers, &v),
        Some("flagged by OpenAI Omni: harassment 35% ≥ 30%".to_string())
    );
}

#[test]
fn test_min_score_of_100_needs_a_full_score() {
    let triggers = only(OpenAiCategory::Hate, CategoryTrigger::MinScorePercent(100));
    assert!(should_moderate(&triggers, &verdict(&[], &[(OpenAiCategory::Hate, 1.0)])).is_some());
    assert!(should_moderate(&triggers, &verdict(&[], &[(OpenAiCategory::Hate, 0.999)])).is_none());
}

#[test]
fn test_a_category_missing_from_the_verdict_does_not_match() {
    let mut triggers = only(OpenAiCategory::Hate, CategoryTrigger::MinScorePercent(1));
    triggers.set(OpenAiCategory::Violence, CategoryTrigger::OpenAiDecides);
    assert_eq!(
        should_moderate(&triggers, &OpenAiModerationResult::default()),
        None
    );
}

#[test]
fn test_one_category_is_enough_and_the_reason_lists_all_that_tripped() {
    let mut triggers = OpenAiCategoryTriggers::default();
    // Set out of order on purpose: the reason follows `OpenAiCategory::ALL`.
    triggers.set(
        OpenAiCategory::Violence,
        CategoryTrigger::MinScorePercent(80),
    );
    triggers.set(OpenAiCategory::Hate, CategoryTrigger::OpenAiDecides);
    triggers.set(OpenAiCategory::Sexual, CategoryTrigger::MinScorePercent(50));
    let v = verdict(
        &[OpenAiCategory::Hate],
        &[
            (OpenAiCategory::Hate, 0.70),
            (OpenAiCategory::Violence, 0.91),
            (OpenAiCategory::Sexual, 0.10),
        ],
    );
    assert_eq!(
        should_moderate(&triggers, &v),
        Some("flagged by OpenAI Omni: hate (OpenAI), violence 91% ≥ 80%".to_string())
    );
}
