use super::{BotDmApplication, describe_actions, lz_compress, quote};
use crate::domain::bot_dm::ports::{
    BotDmReceiver, BotMessenger, Err, Group, GroupId, GroupInvitation, GroupOperations, JoinError,
    Message, ModerationAction, ModerationNotification, ModerationNotificationReceiver, RuleReason,
    TriggerEvent, UserId,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

const KICK: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: false,
};
const KICK_ALL: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: true,
};

// US-D2, US-F2, US-F3
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
        "🛡 I set the member as observer"
    );
    assert_eq!(describe_actions(&[KICK], false), "🛡 I kicked the member");
    assert_eq!(
        describe_actions(&[KICK_ALL], false),
        "🛡 I kicked the member and deleted all their messages"
    );
}

// US-D2
#[test]
fn test_describes_several_actions_as_one_sentence() {
    assert_eq!(
        describe_actions(&[ModerationAction::ModerateMessage, KICK], false),
        "🛡 I moderated the message and kicked the member"
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
        "🛡 I set the member as observer, moderated the message and kicked the member"
    );
}

// US-D4
#[test]
fn test_dry_mode_switches_to_what_would_have_happened() {
    assert_eq!(
        describe_actions(&[ModerationAction::ModerateMessage, KICK], true),
        "🛡 I would moderate the message and kick the member"
    );
    assert_eq!(
        describe_actions(&[KICK_ALL], true),
        "🛡 I would kick the member and delete all their messages"
    );
}

// US-D2, US-F2
#[test]
fn test_describes_a_timed_observer_restriction() {
    assert_eq!(
        describe_actions(
            &[ModerationAction::SetAuthorObserver {
                duration_minutes: 30
            }],
            false
        ),
        "🛡 I set the member as observer for 30 minutes"
    );
    assert_eq!(
        describe_actions(
            &[ModerationAction::SetAuthorObserver {
                duration_minutes: 1
            }],
            true
        ),
        "🛡 I would set the member as observer for 1 minute"
    );
}

#[test]
fn test_quote_keeps_every_line_inside_the_quote() {
    assert_eq!(quote("one line"), "> one line");
    assert_eq!(
        quote("first\n\n# not a heading\nlast"),
        "> first  \n>\n> # not a heading  \n> last"
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
    assert_eq!(sent, vec!["✅ Group settings updated."]);
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

// US-B4: the link carries the group's name and its whole configuration.
#[tokio::test]
async fn test_groups_links_the_whole_config() {
    let (sent, _) = send("/groups").await;

    assert_eq!(sent.len(), 1);
    let link = format!(
        "{EDITOR}#bot_id=5&group=Test%20Group&config={}",
        lz_compress(CONFIG)
    );
    assert!(sent[0].contains(&link), "{}", sent[0]);
}

// US-B4
#[tokio::test]
async fn test_groups_names_the_group_in_bold() {
    let (sent, _) = send("/groups").await;

    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].starts_with("**Test Group**\n[View and Edit Settings]("),
        "{}",
        sent[0]
    );
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

/// A notification about a member's message the bot moderated, with unnamed
/// rules' reasons.
fn message_notification(
    performed: bool,
    message: &str,
    reasons: &[&str],
) -> ModerationNotification {
    ModerationNotification {
        event: TriggerEvent::MessagePosted {
            message: message.to_string(),
        },
        performed,
        member_name: "Bob".to_string(),
        actions: vec![ModerationAction::ModerateMessage],
        reasons: reasons
            .iter()
            .map(|reason| RuleReason {
                rule_name: None,
                reason: reason.to_string(),
            })
            .collect(),
    }
}

// US-D2
#[tokio::test]
async fn test_a_notification_quotes_the_message_and_lists_the_reasons() {
    let messenger = Arc::new(RecordingMessenger::default());
    let app = BotDmApplication::new(
        messenger.clone(),
        Arc::new(OneGroup::default()),
        EDITOR.to_string(),
    );
    let group = Group {
        id: 5,
        name: "Test Group".to_string(),
    };

    app.send_moderation_notification(
        1,
        &group,
        &message_notification(
            true,
            "buy now\n- cheap",
            &["contains word: 'buy'", "has more than 1 lines"],
        ),
    )
    .await
    .unwrap();

    let sent = messenger.sent();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].starts_with("🛡 I moderated the message in **Test Group**."),
        "{}",
        sent[0]
    );
    assert!(
        sent[0].contains("**The message:**\n> buy now  \n> - cheap\n\n"),
        "{}",
        sent[0]
    );
    assert!(
        sent[0].contains("**Reason:**\n- contains word: 'buy'\n- has more than 1 lines\n\n"),
        "{}",
        sent[0]
    );
}

// US-D2
#[tokio::test]
async fn test_a_notification_about_a_message_without_text_says_so() {
    let messenger = Arc::new(RecordingMessenger::default());
    let app = BotDmApplication::new(
        messenger.clone(),
        Arc::new(OneGroup::default()),
        EDITOR.to_string(),
    );
    let group = Group {
        id: 5,
        name: "Test Group".to_string(),
    };

    app.send_moderation_notification(
        1,
        &group,
        &message_notification(false, "", &["contains an image"]),
    )
    .await
    .unwrap();

    assert!(messenger.sent()[0].contains("**The message:**\n_(no text)_\n\n"));
}

// ---- the names of rules, and a join (US-D2, US-D6) ----

const ISSUE: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml";

async fn notified(notification: ModerationNotification) -> String {
    let messenger = Arc::new(RecordingMessenger::default());
    let app = BotDmApplication::new(
        messenger.clone(),
        Arc::new(OneGroup::default()),
        EDITOR.to_string(),
    );
    let group = Group {
        id: 5,
        name: "Crypto Chat".to_string(),
    };
    app.send_moderation_notification(1, &group, &notification)
        .await
        .unwrap();
    let sent = messenger.sent();
    assert_eq!(sent.len(), 1);
    sent[0].clone()
}

fn reason(rule_name: Option<&str>, reason: &str) -> RuleReason {
    RuleReason {
        rule_name: rule_name.map(str::to_string),
        reason: reason.to_string(),
    }
}

fn join_notification(
    performed: bool,
    actions: Vec<ModerationAction>,
    reasons: Vec<RuleReason>,
) -> ModerationNotification {
    ModerationNotification {
        event: TriggerEvent::MemberJoined,
        performed,
        member_name: "Free Casino Bonus".to_string(),
        actions,
        reasons,
    }
}

// US-D2: a matched rule's name is in bold before what matched; a rule without a
// name gives what matched alone.
#[tokio::test]
async fn test_a_notification_names_the_rules_that_matched() {
    let text = notified(ModerationNotification {
        event: TriggerEvent::MessagePosted {
            message: "Buy cheap followers at https://spam.example".to_string(),
        },
        performed: true,
        member_name: "Bob".to_string(),
        actions: vec![
            ModerationAction::SetAuthorObserver {
                duration_minutes: 60,
            },
            ModerationAction::ModerateMessage,
        ],
        reasons: vec![
            reason(Some("Links from newcomers"), "spam.example"),
            reason(None, "contains word: 'buy'"),
        ],
    })
    .await;
    assert_eq!(
        text,
        format!(
            "🛡 I set the member as observer for 60 minutes and moderated the message in **Crypto Chat**.\n\n**The message:**\n> Buy cheap followers at https://spam.example\n\n**Reason:**\n- **Links from newcomers**: spam.example\n- contains word: 'buy'\n\nIf it's a false positive, please [file an issue]({ISSUE})."
        )
    );
}

// US-D6
#[tokio::test]
async fn test_a_join_notification_says_who_joined_and_why() {
    let text = notified(join_notification(
        true,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: false,
        }],
        vec![reason(Some("Spam names"), "name contains word: 'casino'")],
    ))
    .await;
    assert_eq!(
        text,
        format!(
            "🛡 I kicked the member when they joined **Crypto Chat**.\n\n**The member:** Free Casino Bonus\n\n**Reason:**\n- **Spam names**: name contains word: 'casino'\n\nIf it's a false positive, please [file an issue]({ISSUE})."
        )
    );
}

// US-D6: with the mode Dry, what it would have done.
#[tokio::test]
async fn test_a_dry_join_notification_says_what_it_would_have_done() {
    let text = notified(join_notification(
        false,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: false,
        }],
        vec![reason(Some("Spam names"), "name contains word: 'casino'")],
    ))
    .await;
    assert!(
        text.starts_with("🛡 I would kick the member when they joined **Crypto Chat**.\n\n**The member:** Free Casino Bonus\n"),
        "{text}"
    );
}

// US-D6: the actions are named as in a message's notification.
#[tokio::test]
async fn test_a_join_notification_names_the_actions_as_a_messages_does() {
    let observer = notified(join_notification(
        true,
        vec![ModerationAction::SetAuthorObserver {
            duration_minutes: 60,
        }],
        vec![reason(Some("Quarantine newcomers"), "everyone who joins")],
    ))
    .await;
    assert!(
        observer.starts_with(
            "🛡 I set the member as observer for 60 minutes when they joined **Crypto Chat**."
        ),
        "{observer}"
    );
    assert!(
        observer.contains("- **Quarantine newcomers**: everyone who joins\n"),
        "{observer}"
    );

    let kick_all = notified(join_notification(
        true,
        vec![ModerationAction::KickAuthor {
            delete_all_messages: true,
        }],
        vec![reason(None, "name links to spam.example")],
    ))
    .await;
    assert!(
        kick_all.starts_with(
            "🛡 I kicked the member and deleted all their messages when they joined **Crypto Chat**."
        ),
        "{kick_all}"
    );
    assert!(
        kick_all.contains("**Reason:**\n- name links to spam.example\n\n"),
        "{kick_all}"
    );
}
