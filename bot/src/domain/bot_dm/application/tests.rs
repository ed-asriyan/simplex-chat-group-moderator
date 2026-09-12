use super::describe_actions;
use crate::domain::bot_dm::ports::ModerationAction;

const KICK: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: false,
};
const KICK_ALL: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: true,
};

#[test]
fn test_describes_a_single_action() {
    assert_eq!(
        describe_actions(&[ModerationAction::ModerateMessage], false),
        "🛡 I moderated the message"
    );
    assert_eq!(
        describe_actions(&[ModerationAction::SetAuthorObserver], false),
        "🛡 I set the author as observer"
    );
    assert_eq!(describe_actions(&[KICK], false), "🛡 I kicked the author");
    assert_eq!(
        describe_actions(&[KICK_ALL], false),
        "🛡 I kicked the author and deleted all their messages"
    );
}

#[test]
fn test_describes_several_actions_as_one_sentence() {
    assert_eq!(
        describe_actions(&[ModerationAction::ModerateMessage, KICK], false),
        "🛡 I moderated the message and kicked the author"
    );
    assert_eq!(
        describe_actions(
            &[
                ModerationAction::SetAuthorObserver,
                ModerationAction::ModerateMessage,
                KICK
            ],
            false
        ),
        "🛡 I set the author as observer, moderated the message and kicked the author"
    );
}

#[test]
fn test_dry_mode_switches_to_what_would_have_happened() {
    assert_eq!(
        describe_actions(&[ModerationAction::ModerateMessage, KICK], true),
        "🛡 I would moderate the message and kick the author"
    );
    assert_eq!(
        describe_actions(&[KICK_ALL], true),
        "🛡 I would kick the author and delete all their messages"
    );
}
