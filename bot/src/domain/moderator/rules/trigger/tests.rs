use super::*;
use crate::domain::moderator::ports::actions::{KickAuthor, ModerateMessage};
use crate::domain::moderator::ports::conditions::{ContainsWords, MemberNameContainsWords};
use crate::domain::moderator::ports::{
    JoinAction, JoinCondition, MessageAction, MessageCondition, ModerationAction,
    ModerationCondition,
};

fn words(keyword: &str) -> ModerationCondition {
    ModerationCondition::ContainsWords(ContainsWords {
        keywords: vec![keyword.to_string()],
    })
}

fn name_words(keyword: &str) -> ModerationCondition {
    ModerationCondition::MemberNameContainsWords(MemberNameContainsWords {
        keywords: vec![keyword.to_string()],
    })
}

// US-D1, US-D6: the trigger's name in the JSON and in the database.
#[test]
fn test_a_trigger_has_a_name_that_names_it_back() {
    assert_eq!(Trigger::MessagePosted.name(), "message_posted");
    assert_eq!(Trigger::MemberJoined.name(), "member_joined");
    for trigger in [Trigger::MessagePosted, Trigger::MemberJoined] {
        assert_eq!(Trigger::from_name(trigger.name()), Some(trigger));
        assert_eq!(
            serde_json::to_value(trigger).unwrap(),
            serde_json::json!(trigger.name())
        );
    }
    assert_eq!(Trigger::from_name("something_else"), None);
    assert_eq!(Trigger::default(), Trigger::MessagePosted);
}

// US-C2
#[test]
fn test_a_leaf_a_trigger_does_not_take_is_refused_in_words() {
    assert_eq!(
        not_taken("Moderate the message", Trigger::MemberJoined).to_string(),
        "'Moderate the message' does not work in a rule for when someone joins."
    );
    assert_eq!(
        not_taken("Some condition", Trigger::MessagePosted).to_string(),
        "'Some condition' does not work in a rule for when someone sends a message."
    );
}

// US-C2: a message rule takes every leaf; a join rule only the name conditions.
#[test]
fn test_the_message_trigger_takes_the_message_conditions_and_the_name_conditions() {
    assert!(MessageCondition::try_from(words("spam")).is_ok());
    assert!(MessageCondition::try_from(name_words("casino")).is_ok());
}

#[test]
fn test_the_join_trigger_takes_only_the_name_conditions() {
    assert!(JoinCondition::try_from(name_words("casino")).is_ok());
    assert!(JoinCondition::try_from(words("spam")).is_err());
}

// US-E27, US-E28, US-E29: the combinations work in a rule for joins too, with
// what its trigger takes inside them.
#[test]
fn test_the_combinations_are_the_triggers_own() {
    let tree = ModerationCondition::All {
        conditions: vec![
            name_words("casino"),
            ModerationCondition::Not {
                condition: Box::new(name_words("friend")),
            },
        ],
    };
    let join = JoinCondition::try_from(tree.clone()).unwrap();
    assert_eq!(ModerationCondition::from(join), tree);

    let wrong = ModerationCondition::Any {
        conditions: vec![
            name_words("casino"),
            ModerationCondition::Not {
                condition: Box::new(words("spam")),
            },
        ],
    };
    assert!(JoinCondition::try_from(wrong).is_err());
}

// US-F1, US-F2, US-F3
#[test]
fn test_the_actions_a_trigger_takes() {
    let moderate = ModerationAction::ModerateMessage(ModerateMessage {});
    let kick = ModerationAction::KickAuthor(KickAuthor {
        delete_all_messages: true,
    });
    assert!(MessageAction::try_from(moderate).is_ok());
    assert!(MessageAction::try_from(kick).is_ok());
    assert!(JoinAction::try_from(kick).is_ok());
    assert!(JoinAction::try_from(moderate).is_err());
    assert_eq!(
        ModerationAction::from(JoinAction::try_from(kick).unwrap()),
        kick
    );
}
