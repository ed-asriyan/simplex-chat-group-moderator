//! The registry as a whole: what holds for every action, and carrying out a
//! planned list.

use super::actions::{KickAuthor, ModerateMessage, SendMemberMessage, SetAuthorObserver};
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

fn send(text: &str) -> ModerationAction {
    ModerationAction::SendMemberMessage(SendMemberMessage {
        text: text.to_string(),
    })
}

async fn execute(ports: &Recorder, actions: &[ModerationAction]) -> ActionOutcome {
    let action_ports = ActionPorts {
        group_moderator: ports,
        restores: ports,
    };
    let actions: Vec<MessageAction> = actions
        .iter()
        .map(|action| MessageAction::try_from(action.clone()).unwrap())
        .collect();
    execute_message_actions(&message(), &actions, &action_ports)
        .await
        .unwrap()
}

#[test]
fn test_type_name_is_the_serde_tag() {
    for action in [MODERATE, OBSERVER, KICK] {
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], action.type_name());
    }
}

// US-F4: the message to the member goes before every other action, while the member
// can still get it.
#[test]
fn test_the_message_to_the_member_runs_before_every_other_action() {
    let message = send("hi").execution_rank();
    for other in [OBSERVER, MODERATE, KICK, KICK_ALL] {
        assert!(message < other.execution_rank(), "{other:?}");
    }
}

// US-F5: a message to the member is written as its type and its text.
#[test]
fn test_a_message_to_the_member_is_written_as_its_type_and_its_text() {
    let action = send("Hi\n_there_");
    let json = serde_json::to_value(&action).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "type": "SendMemberMessage", "text": "Hi\n_there_" })
    );
    assert_eq!(action.type_name(), "SendMemberMessage");
    assert_eq!(
        serde_json::from_value::<ModerationAction>(json).unwrap(),
        action
    );
}

// US-F6, US-F4: the planned list, the message first, is carried out in that order.
#[tokio::test]
async fn test_the_message_to_the_member_is_sent_before_the_member_is_restricted_or_removed() {
    let ports = Recorder::default();
    execute(&ports, &[send("hi"), OBSERVER, MODERATE, KICK]).await;
    assert_eq!(
        ports.calls(),
        vec![
            "send 42 in 1: hi",
            "make 42 Observer in 1",
            "cancel the restore of 42 in 1",
            "delete message 7 in 1",
            "kick 42 from 1, delete all: false",
            "cancel the restore of 42 in 1",
        ]
    );
}

// US-F6: a message that cannot reach the member does not stop the actions after it.
#[tokio::test]
async fn test_a_message_that_cannot_reach_the_member_does_not_stop_the_actions_after_it() {
    let ports = Recorder::failing_sends();
    // `execute` fails the test if the list fails.
    let outcome = execute(&ports, &[send("hi"), MODERATE, KICK]).await;
    assert_eq!(
        ports.calls(),
        vec![
            "send 42 in 1: hi",
            "delete message 7 in 1",
            "kick 42 from 1, delete all: false",
            "cancel the restore of 42 in 1",
        ]
    );
    assert!(outcome.message_deleted);
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
