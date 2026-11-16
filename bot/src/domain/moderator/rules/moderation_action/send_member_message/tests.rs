use super::*;
use crate::domain::moderator::rules::moderation_action::Silence;
use crate::domain::moderator::rules::moderation_action::fakes::Recorder;

fn send(text: &str) -> SendMemberMessage {
    SendMemberMessage {
        text: text.to_string(),
    }
}

// US-F5, US-C2: the text is at most 1000 characters.
#[test]
fn test_the_limit_is_1000_characters() {
    assert_eq!(MAX_MEMBER_MESSAGE_CHARS, 1000);
}

// US-F5: the action is titled as the owner finds it in the editor, which is how a
// rule for joins is refused it (US-C2).
#[test]
fn test_the_action_is_titled_as_the_editor_titles_it() {
    assert_eq!(send("hi").title(), "Send the member a private message");
}

// US-F5: sent "exactly as I wrote it": nothing is trimmed, replaced or filled in.
#[test]
fn test_a_text_is_kept_exactly_as_the_owner_wrote_it() {
    let written = "  **Read the rules** first.\n_No spam_, please: {name} and {reason} stay as written.\n";
    let mut action = send(written);
    action.normalize_and_validate().unwrap();
    assert_eq!(action.text, written);
}

// US-F5, US-C2: the text is required; one of spaces alone is as good as none.
#[test]
fn test_an_empty_text_is_refused() {
    for text in ["", " ", "\n\t  \n"] {
        let err = send(text).normalize_and_validate().unwrap_err().to_string();
        assert_eq!(
            err,
            "A private message to the member is empty. Write it, or remove the action.",
            "{text:?}"
        );
    }
}

// US-F5, US-C2: at most 1000 characters, counted as characters, not as bytes.
#[test]
fn test_a_text_is_at_most_1000_characters_counted_as_characters() {
    for filler in ["x", "я", "🙂"] {
        send(&filler.repeat(1000)).normalize_and_validate().unwrap();
        let err = send(&filler.repeat(1001))
            .normalize_and_validate()
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "A private message to the member is longer than 1000 characters.",
            "{filler}"
        );
    }
}

// US-F4: sending a message achieves something no other action does, so none of
// them stands in for it.
#[test]
fn test_a_message_to_the_member_achieves_something_of_its_own() {
    let effect = send("hi").effect();
    assert_ne!(effect, Effect::default());
    assert!(!Effect::default().covers(&effect));
}

// US-F4: the member gets one message, the first matched rule's: one message covers
// another, whatever either says.
#[test]
fn test_one_message_to_the_member_covers_another_whatever_its_text() {
    assert!(send("first").effect().covers(&send("second").effect()));
    assert!(send("second").effect().covers(&send("first").effect()));
}

// US-F4: and it covers nothing else, nor does the strongest of the others cover it.
#[test]
fn test_a_message_to_the_member_covers_nothing_else_and_is_covered_by_nothing_else() {
    let strongest = Effect {
        message_deleted: true,
        history_deleted: true,
        author_silenced: Silence::Forever,
        author_removed: true,
        ..Effect::default()
    };
    assert!(!send("hi").effect().covers(&strongest));
    assert!(!strongest.covers(&send("hi").effect()));
}

// US-F6: the author is sent the text, in the group the message was posted in.
#[tokio::test]
async fn test_the_author_is_sent_the_text_in_the_group_it_was_posted_in() {
    let ports = Recorder::default();
    let report = ports
        .execute(&send("Your message was removed."))
        .await
        .unwrap();
    // The message every test acts on is message 7 by member 42 in group 1; nothing
    // else is done to the member.
    assert_eq!(
        ports.calls(),
        vec!["send 42 in 1: Your message was removed."]
    );
    assert!(report.bookkeeping_error.is_none());
}

// US-F5, US-F6: the text goes out as the owner wrote it, Markdown and lines included.
#[tokio::test]
async fn test_the_text_goes_out_as_written() {
    let text = "**Read the rules** first.\n_No spam_ here.";
    let ports = Recorder::default();
    ports.execute(&send(text)).await.unwrap();
    assert_eq!(ports.calls(), vec![format!("send 42 in 1: {text}")]);
}

// US-F6: a message that cannot reach the member is not the action's failure, so
// the rule's other actions are carried out all the same.
#[tokio::test]
async fn test_a_message_that_cannot_reach_the_author_does_not_fail_the_action() {
    let ports = Recorder::failing_sends();
    assert!(ports.execute(&send("hi")).await.is_ok());
    // It was tried, once.
    assert_eq!(ports.calls(), vec!["send 42 in 1: hi"]);
}
