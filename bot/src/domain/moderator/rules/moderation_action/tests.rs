//! The registry as a whole: what holds for every action, and carrying out a
//! planned list.

use super::actions::{KickAuthor, ModerateMessage, SetAuthorObserver};
use super::fakes::{Recorder, message};
use super::*;

const MODERATE: ModerationAction = ModerationAction::ModerateMessage(ModerateMessage {});
const OBSERVER: ModerationAction = ModerationAction::SetAuthorObserver(SetAuthorObserver {
    duration_minutes: 0,
});
const KICK: ModerationAction = ModerationAction::KickAuthor(KickAuthor {
    delete_all_messages: false,
});
const KICK_ALL: ModerationAction = ModerationAction::KickAuthor(KickAuthor {
    delete_all_messages: true,
});

async fn execute(ports: &Recorder, actions: &[ModerationAction]) -> ActionOutcome {
    let action_ports = ActionPorts {
        group_moderator: ports,
        restores: ports,
    };
    execute_actions(&message(), actions, &action_ports)
        .await
        .unwrap()
}

#[test]
fn test_type_name_is_the_serde_tag() {
    for action in [MODERATE, OBSERVER, KICK] {
        let json = serde_json::to_value(action).unwrap();
        assert_eq!(json["type"], action.type_name());
    }
}

#[test]
fn test_every_action_has_its_own_place_in_the_execution_order() {
    assert!(OBSERVER.execution_rank() < MODERATE.execution_rank());
    assert!(MODERATE.execution_rank() < KICK.execution_rank());
}

#[tokio::test]
async fn test_actions_run_in_the_order_given() {
    let ports = Recorder::default();
    execute(&ports, &[OBSERVER, MODERATE, KICK]).await;
    assert_eq!(
        ports.calls(),
        vec![
            "make 42 Observer in 1",
            "cancel the restore of 42 in 1",
            "delete message 7 in 1",
            "kick 42 from 1, delete all: false",
            "cancel the restore of 42 in 1",
        ]
    );
}

#[tokio::test]
async fn test_the_message_is_deleted_by_whichever_action_deletes_it() {
    let ports = Recorder::default();
    assert!(execute(&ports, &[MODERATE]).await.message_deleted);
    assert!(execute(&ports, &[KICK_ALL]).await.message_deleted);
    assert!(!execute(&ports, &[OBSERVER, KICK]).await.message_deleted);
}

#[tokio::test]
async fn test_a_bookkeeping_failure_does_not_stop_the_actions_after_it() {
    let ports = Recorder::failing_restores();
    let outcome = execute(&ports, &[OBSERVER, MODERATE]).await;
    assert_eq!(ports.calls().last().unwrap(), "delete message 7 in 1");
    assert_eq!(
        outcome.bookkeeping_error.unwrap().to_string(),
        "restore bookkeeping failed"
    );
}
