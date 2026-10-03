use super::*;
use crate::domain::moderator::rules::moderation_action::fakes::{Recorder, at};

fn observer(duration_minutes: u32) -> SetAuthorObserver {
    SetAuthorObserver { duration_minutes }
}

#[test]
fn test_duration_is_capped_at_thirty_days() {
    assert!(observer(MAX_OBSERVER_DURATION_MINUTES)
        .normalize_and_validate()
        .is_ok());
    let error = observer(MAX_OBSERVER_DURATION_MINUTES + 1)
        .normalize_and_validate()
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Observer duration too long: 43201 minutes, maximum is 43200 (30 days)"
    );
}

#[test]
fn test_indefinitely_is_not_a_long_timer() {
    // `0` is allowed whatever the cap, and is the strictest restriction.
    assert!(observer(0).normalize_and_validate().is_ok());
    assert_eq!(observer(0).effect().author_silenced, Silence::Forever);
    assert_eq!(
        observer(10).effect().author_silenced,
        Silence::For { minutes: 10 }
    );
}

#[test]
fn test_an_observer_only_silences_the_author() {
    assert_eq!(
        observer(10).effect(),
        Effect {
            author_silenced: Silence::For { minutes: 10 },
            ..Effect::default()
        }
    );
}

#[tokio::test]
async fn test_a_timed_restriction_schedules_the_restore() {
    let ports = Recorder::default();
    let report = ports.execute(&observer(30)).await.unwrap();
    assert_eq!(
        ports.calls(),
        vec![
            "make 42 Observer in 1".to_string(),
            format!("restore 42 in 1 at {}", at(30)),
        ]
    );
    assert!(report.bookkeeping_error.is_none());
}

#[tokio::test]
async fn test_an_indefinite_restriction_cancels_an_earlier_restore() {
    let ports = Recorder::default();
    ports.execute(&observer(0)).await.unwrap();
    assert_eq!(
        ports.calls(),
        vec!["make 42 Observer in 1", "cancel the restore of 42 in 1"]
    );
}

#[tokio::test]
async fn test_a_restore_that_cannot_be_kept_is_reported_not_failed() {
    let ports = Recorder::failing_restores();
    let report = ports.execute(&observer(30)).await.unwrap();
    assert_eq!(
        report.bookkeeping_error.unwrap().to_string(),
        "restore bookkeeping failed"
    );
}
