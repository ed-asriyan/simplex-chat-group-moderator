use super::*;
use crate::domain::moderator::rules::moderation_action::fakes::Recorder;

#[test]
fn test_moderating_deletes_the_message_and_nothing_else() {
    assert_eq!(
        ModerateMessage {}.effect(),
        Effect {
            message_deleted: true,
            ..Effect::default()
        }
    );
}

#[tokio::test]
async fn test_moderating_deletes_the_message_that_matched() {
    let ports = Recorder::default();
    let report = ports.execute(&ModerateMessage {}).await.unwrap();
    assert_eq!(ports.calls(), vec!["delete message 7 in 1"]);
    assert!(report.bookkeeping_error.is_none());
}
