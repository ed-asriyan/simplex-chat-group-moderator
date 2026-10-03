use super::*;
use crate::domain::moderator::rules::moderation_action::fakes::Recorder;

fn kick(delete_all_messages: bool) -> KickAuthor {
    KickAuthor {
        delete_all_messages,
    }
}

#[test]
fn test_a_kick_removes_and_silences_the_author_for_good() {
    assert_eq!(
        kick(false).effect(),
        Effect {
            author_silenced: Silence::Forever,
            author_removed: true,
            ..Effect::default()
        }
    );
}

#[test]
fn test_deleting_every_message_deletes_the_one_that_matched_too() {
    let effect = kick(true).effect();
    assert!(effect.history_deleted);
    assert!(effect.message_deleted);
}

#[tokio::test]
async fn test_a_kick_cancels_the_restore_owed_to_the_author() {
    let ports = Recorder::default();
    let report = ports.execute(&kick(true)).await.unwrap();
    assert_eq!(
        ports.calls(),
        vec![
            "kick 42 from 1, delete all: true",
            "cancel the restore of 42 in 1"
        ]
    );
    assert!(report.bookkeeping_error.is_none());
}

#[tokio::test]
async fn test_a_restore_that_cannot_be_cancelled_is_reported_not_failed() {
    let ports = Recorder::failing_restores();
    let report = ports.execute(&kick(false)).await.unwrap();
    assert_eq!(
        report.bookkeeping_error.unwrap().to_string(),
        "restore bookkeeping failed"
    );
}
