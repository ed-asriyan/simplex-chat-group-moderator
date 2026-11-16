use super::*;
use crate::domain::bot_dm::ports::Err as BotDmErr;
use crate::domain::moderator::ports::RuleReason;
use crate::domain::moderator::ports::actions::{
    KickAuthor, ModerateMessage, SendMemberMessage, SetAuthorObserver,
};
use std::sync::Mutex;

/// What the owner's side was told, whole.
#[derive(Default)]
struct Receiver {
    told: Mutex<Vec<(i64, (i64, String), ModerationNotification)>>,
}

#[async_trait]
impl ModerationNotificationReceiver for Receiver {
    async fn send_moderation_notification(
        &self,
        user_id: i64,
        group: &BotDmGroup,
        notification: &ModerationNotification,
    ) -> Result<(), BotDmErr> {
        self.told.lock().unwrap().push((
            user_id,
            (group.id, group.name.clone()),
            notification.clone(),
        ));
        Ok(())
    }
}

fn group() -> ModGroup {
    ModGroup {
        id: 10,
        owner_id: 100,
        name: "Crypto Chat".to_string(),
    }
}

async fn routed(report: ModerationReport) -> (i64, (i64, String), ModerationNotification) {
    let receiver = Arc::new(Receiver::default());
    let router = ModerationNotificationRouter::new();
    router.set_receiver(receiver.clone());
    router
        .notify_moderation(100, &group(), &report)
        .await
        .unwrap();
    let mut told = receiver.told.lock().unwrap();
    assert_eq!(told.len(), 1);
    told.remove(0)
}

// US-D2, US-D4: every action the moderator names reaches the owner's side as its
// own, the message to the member included, in the order the moderator gave, what
// was carried out apart from what would have been.
#[tokio::test]
async fn test_every_action_is_translated_in_its_order() {
    let (user, group, told) = routed(ModerationReport {
        event: ModTriggerEvent::MessagePosted {
            message: "spam".to_string(),
        },
        member_name: "Bob".to_string(),
        performed: vec![
            ModAction::SendMemberMessage(SendMemberMessage {
                text: "Your message was removed.".to_string(),
            }),
            ModAction::SetAuthorObserver(SetAuthorObserver {
                duration_minutes: 10,
            }),
            ModAction::ModerateMessage(ModerateMessage {}),
        ],
        would_perform: vec![
            ModAction::SendMemberMessage(SendMemberMessage {
                text: "Another text.".to_string(),
            }),
            ModAction::KickAuthor(KickAuthor {
                delete_all_messages: true,
            }),
        ],
        reasons: vec![RuleReason {
            rule_name: Some("Banned words".to_string()),
            reason: "contains word: 'spam'".to_string(),
            actions_on: true,
        }],
    })
    .await;

    assert_eq!(user, 100);
    assert_eq!(group, (10, "Crypto Chat".to_string()));
    assert_eq!(
        told.performed,
        vec![
            BotDmAction::SendMemberMessage,
            BotDmAction::SetAuthorObserver {
                duration_minutes: 10
            },
            BotDmAction::ModerateMessage,
        ]
    );
    assert_eq!(
        told.would_perform,
        vec![
            BotDmAction::SendMemberMessage,
            BotDmAction::KickAuthor {
                delete_all_messages: true
            },
        ]
    );
    assert_eq!(told.member_name, "Bob");
    assert_eq!(
        told.event,
        TriggerEvent::MessagePosted {
            message: "spam".to_string()
        }
    );
    assert_eq!(told.reasons.len(), 1);
}
