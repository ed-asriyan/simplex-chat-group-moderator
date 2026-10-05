use super::{BotDmApplication, describe_actions, lz_compress};
use crate::domain::bot_dm::ports::{
    BotDmReceiver, BotMessenger, Err, Group, GroupId, GroupInvitation, GroupOperations, JoinError,
    Message, ModerationAction, UserId,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

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
        describe_actions(
            &[ModerationAction::SetAuthorObserver {
                duration_minutes: 0
            }],
            false
        ),
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
                ModerationAction::SetAuthorObserver {
                    duration_minutes: 0
                },
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

#[test]
fn test_describes_a_timed_observer_restriction() {
    assert_eq!(
        describe_actions(
            &[ModerationAction::SetAuthorObserver {
                duration_minutes: 30
            }],
            false
        ),
        "🛡 I set the author as observer for 30 minutes"
    );
    assert_eq!(
        describe_actions(
            &[ModerationAction::SetAuthorObserver {
                duration_minutes: 1
            }],
            true
        ),
        "🛡 I would set the author as observer for 1 minute"
    );
}

const EDITOR: &str = "https://editor.example";
const CONFIG: &str = r#"{"mode":"dry","rules":[]}"#;

#[derive(Default)]
struct RecordingMessenger {
    sent: Mutex<Vec<String>>,
}

impl RecordingMessenger {
    fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }
}

#[async_trait]
impl BotMessenger for RecordingMessenger {
    async fn send_dm(&self, _user_id: &UserId, text: &str) -> Result<(), Err> {
        self.sent.lock().unwrap().push(text.to_string());
        Ok(())
    }
}

/// One group, 5, whose configuration is [`CONFIG`]; remembers every
/// configuration it is handed.
#[derive(Default)]
struct OneGroup {
    saved: Mutex<Vec<(GroupId, String)>>,
}

#[async_trait]
impl GroupOperations for OneGroup {
    async fn try_join_group(
        &self,
        _user_id: UserId,
        _invitation: &GroupInvitation,
    ) -> Result<Group, JoinError> {
        Err("not in these tests".into())
    }

    async fn get_groups(&self, _user_id: UserId) -> Result<Vec<Group>, Err> {
        Ok(vec![Group {
            id: 5,
            name: "Test Group".to_string(),
        }])
    }

    async fn set_config_json(
        &self,
        _user_id: UserId,
        group_id: GroupId,
        json: &str,
    ) -> Result<(), Err> {
        self.saved
            .lock()
            .unwrap()
            .push((group_id, json.to_string()));
        Ok(())
    }

    async fn get_config_json(&self, _user_id: UserId, _group_id: GroupId) -> Result<String, Err> {
        Ok(CONFIG.to_string())
    }
}

async fn send(text: &str) -> (Vec<String>, Vec<(GroupId, String)>) {
    let messenger = Arc::new(RecordingMessenger::default());
    let groups = Arc::new(OneGroup::default());
    let app = BotDmApplication::new(messenger.clone(), groups.clone(), EDITOR.to_string());
    let message = Message {
        text: text.to_string(),
        reply_to_message: None,
    };
    app.handle_dm(1, &message).await.unwrap();
    let saved = groups.saved.lock().unwrap().clone();
    (messenger.sent(), saved)
}

#[tokio::test]
async fn test_a_sent_editor_link_saves_the_whole_config() {
    let link = format!(
        "[Settings]({EDITOR}#bot_id=5&config={})",
        lz_compress(CONFIG)
    );

    let (sent, saved) = send(&link).await;

    assert_eq!(saved, vec![(5, CONFIG.to_string())]);
    assert_eq!(sent, vec!["Group settings updated successfully."]);
}

#[tokio::test]
async fn test_a_link_without_a_config_saves_nothing() {
    let link = format!("{EDITOR}#bot_id=5&rules={}", lz_compress("[]"));

    let (sent, saved) = send(&link).await;

    assert!(saved.is_empty());
    assert_eq!(
        sent,
        vec!["Unknown command. Send /help to see what I can do."]
    );
}

#[tokio::test]
async fn test_the_mode_is_no_longer_a_command() {
    for command in ["/dry_on_5", "/dry_off_5", "/notify_on_5", "/notify_off_5"] {
        let (sent, saved) = send(command).await;

        assert!(saved.is_empty(), "{command}");
        assert_eq!(
            sent,
            vec!["Unknown command. Send /help to see what I can do."],
            "{command}"
        );
    }
}

#[tokio::test]
async fn test_groups_links_the_whole_config() {
    let (sent, _) = send("/groups").await;

    assert_eq!(sent.len(), 1);
    let link = format!("{EDITOR}#bot_id=5&config={}", lz_compress(CONFIG));
    assert!(sent[0].contains(&link), "{}", sent[0]);
}

/// Counts join attempts; every attempt fails.
#[derive(Default)]
struct CountingJoins {
    attempts: Mutex<u32>,
}

#[async_trait]
impl GroupOperations for CountingJoins {
    async fn try_join_group(
        &self,
        _user_id: UserId,
        _invitation: &GroupInvitation,
    ) -> Result<Group, JoinError> {
        *self.attempts.lock().unwrap() += 1;
        Err("refused".into())
    }

    async fn get_groups(&self, _user_id: UserId) -> Result<Vec<Group>, Err> {
        Ok(vec![])
    }

    async fn set_config_json(&self, _: UserId, _: GroupId, _: &str) -> Result<(), Err> {
        Ok(())
    }

    async fn get_config_json(&self, _: UserId, _: GroupId) -> Result<String, Err> {
        Ok(CONFIG.to_string())
    }
}

async fn invite(is_moderator_or_higher: bool) -> (Vec<String>, u32) {
    let messenger = Arc::new(RecordingMessenger::default());
    let groups = Arc::new(CountingJoins::default());
    let app = BotDmApplication::new(messenger.clone(), groups.clone(), EDITOR.to_string());
    let invitation = GroupInvitation {
        group_id: 5,
        group_name: "Test Group".to_string(),
        is_moderator_or_higher,
    };
    app.handle_group_invitation(1, &invitation).await.unwrap();
    let attempts = *groups.attempts.lock().unwrap();
    (messenger.sent(), attempts)
}

#[tokio::test]
async fn test_an_invitation_below_moderator_is_not_joined() {
    let (sent, attempts) = invite(false).await;

    assert_eq!(attempts, 0);
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("moderator"));
}

#[tokio::test]
async fn test_an_invitation_as_moderator_is_joined() {
    let (_, attempts) = invite(true).await;

    assert_eq!(attempts, 1);
}
