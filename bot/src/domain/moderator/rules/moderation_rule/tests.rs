use super::*;
use crate::domain::moderator::ports::actions::{KickAuthor, ModerateMessage, SetAuthorObserver};
use crate::domain::moderator::ports::conditions::{
    AuthorHitsMessageRateLimit, ContainsWords, FlaggedByOpenRouterInstruction,
    MemberNameContainsWords,
};
use crate::domain::moderator::rules::test_support::{join_rule, message_rule};
use serde_json::json;

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

fn moderate() -> ModerationAction {
    ModerationAction::ModerateMessage(ModerateMessage {})
}

fn kick() -> ModerationAction {
    ModerationAction::KickAuthor(KickAuthor {
        delete_all_messages: false,
    })
}

fn observer(minutes: u32) -> ModerationAction {
    ModerationAction::SetAuthorObserver(SetAuthorObserver {
        duration_minutes: minutes,
    })
}

fn parse(value: serde_json::Value) -> Result<ModerationRule, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

// ---- the JSON ----

#[test]
fn test_deserialize_rule_with_moderate_message_action() {
    let rule = parse(json!({
        "actions": [{ "type": "ModerateMessage" }],
        "condition": { "type": "ContainsWords", "keywords": ["spam", "ad"] }
    }))
    .unwrap();
    let draft = RuleDraft::from(rule);
    assert_eq!(draft.actions, vec![moderate()]);
    assert_eq!(
        draft.condition,
        Some(ModerationCondition::ContainsWords(ContainsWords {
            keywords: vec!["spam".to_string(), "ad".to_string()]
        }))
    );
}

#[test]
fn test_deserialize_rule_with_kick_author_action() {
    let rule = parse(json!({
        "actions": [{ "type": "KickAuthor", "delete_all_messages": true }],
        "condition": { "type": "ContainsWords", "keywords": ["malware"] }
    }))
    .unwrap();
    assert_eq!(
        RuleDraft::from(rule).actions,
        vec![ModerationAction::KickAuthor(KickAuthor {
            delete_all_messages: true
        })]
    );
    // An omitted flag leaves the author's history alone.
    let rule = parse(json!({
        "actions": [{ "type": "KickAuthor" }],
        "condition": { "type": "ContainsWords", "keywords": ["malware"] }
    }))
    .unwrap();
    assert_eq!(RuleDraft::from(rule).actions, vec![kick()]);
}

#[test]
fn test_deserialize_rule_with_several_actions_keeps_their_order() {
    let rule = parse(json!({
        "actions": [{ "type": "SetAuthorObserver" }, { "type": "ModerateMessage" }],
        "condition": { "type": "ContainsWords", "keywords": ["spam"] }
    }))
    .unwrap();
    assert_eq!(RuleDraft::from(rule).actions, vec![observer(0), moderate()]);
}

#[test]
fn test_serialization_roundtrip() {
    let rule = message_rule(vec![moderate(), kick()], words("banned message"));
    let back: ModerationRule =
        serde_json::from_str(&serde_json::to_string(&rule).unwrap()).unwrap();
    assert_eq!(rule, back);
    let join = join_rule(vec![kick()], Some(name_words("casino")));
    let back: ModerationRule =
        serde_json::from_str(&serde_json::to_string(&join).unwrap()).unwrap();
    assert_eq!(join, back);
}

// US-D1: a rule without a trigger is a message rule; without `enabled`, on.
#[test]
fn test_a_rule_without_a_trigger_or_a_switch_is_a_message_rule_that_is_on() {
    let rule = parse(json!({
        "actions": [{ "type": "ModerateMessage" }],
        "condition": { "type": "ContainsWords", "keywords": ["spam"] }
    }))
    .unwrap();
    assert_eq!(rule.trigger(), Trigger::MessagePosted);
    assert!(rule.enabled);
    assert_eq!(rule.name, None);
}

// US-G13: the switch and the name survive the JSON.
#[test]
fn test_a_rule_keeps_its_name_and_its_switch() {
    let rule = parse(json!({
        "name": "Flood",
        "trigger": "message_posted",
        "enabled": false,
        "actions": [{ "type": "ModerateMessage" }],
        "condition": { "type": "ContainsWords", "keywords": ["spam"] }
    }))
    .unwrap();
    assert_eq!(rule.name.as_deref(), Some("Flood"));
    assert!(!rule.enabled);
    let written = serde_json::to_value(&rule).unwrap();
    assert_eq!(written["name"], "Flood");
    assert_eq!(written["enabled"], false);
    assert_eq!(written["trigger"], "message_posted");
}

// US-D6: a rule for joins may have no condition.
#[test]
fn test_a_join_rule_may_have_no_condition() {
    let rule = parse(json!({
        "trigger": "member_joined",
        "actions": [{ "type": "SetAuthorObserver", "duration_minutes": 60 }]
    }))
    .unwrap();
    assert_eq!(rule.trigger(), Trigger::MemberJoined);
    assert_eq!(RuleDraft::from(rule).condition, None);
}

// ---- what a trigger takes (US-C2) ----

// US-C2
#[test]
fn test_a_message_rule_with_no_condition_is_refused() {
    let err = parse(json!({
        "actions": [{ "type": "ModerateMessage" }]
    }))
    .unwrap_err();
    assert!(
        err.contains(
            "A rule for when someone sends a message has no condition. Add one, or remove the rule."
        ),
        "{err}"
    );
}

// US-C2, US-F1
#[test]
fn test_moderate_the_message_does_not_work_in_a_rule_for_joins() {
    let err = parse(json!({
        "trigger": "member_joined",
        "actions": [{ "type": "ModerateMessage" }]
    }))
    .unwrap_err();
    assert!(
        err.contains("'Moderate the message' does not work in a rule for when someone joins."),
        "{err}"
    );
}

// US-C2: a condition that reads the message does not work in a rule for joins.
#[test]
fn test_a_message_condition_does_not_work_in_a_rule_for_joins() {
    let err = parse(json!({
        "trigger": "member_joined",
        "condition": { "type": "ContainsWords", "keywords": ["spam"] },
        "actions": [{ "type": "KickAuthor" }]
    }))
    .unwrap_err();
    assert!(
        err.contains("does not work in a rule for when someone joins."),
        "{err}"
    );
}

// US-C2: the same one nested in a combination is found too.
#[test]
fn test_a_message_condition_nested_in_a_combination_is_found() {
    let err = parse(json!({
        "trigger": "member_joined",
        "condition": { "type": "All", "conditions": [
            { "type": "MemberNameContainsWords", "keywords": ["casino"] },
            { "type": "Not", "condition": { "type": "ContainsWords", "keywords": ["spam"] } }
        ]},
        "actions": [{ "type": "KickAuthor" }]
    }))
    .unwrap_err();
    assert!(
        err.contains("does not work in a rule for when someone joins."),
        "{err}"
    );
}

// US-E31, US-E32, US-F2, US-F3: what works with both triggers.
#[test]
fn test_the_name_conditions_and_the_member_actions_work_in_both_triggers() {
    for trigger in ["message_posted", "member_joined"] {
        let rule = parse(json!({
            "trigger": trigger,
            "condition": { "type": "Any", "conditions": [
                { "type": "MemberNameContainsWords", "keywords": ["casino"] },
                { "type": "MemberNameContainsLinksOutsideList", "domains": [] }
            ]},
            "actions": [{ "type": "SetAuthorObserver" }, { "type": "KickAuthor" }]
        }));
        assert!(rule.is_ok(), "{trigger}: {rule:?}");
    }
}

// US-F1: a message rule takes the action on the message.
#[test]
fn test_a_message_rule_takes_moderate_the_message() {
    assert!(
        parse(json!({
            "actions": [{ "type": "ModerateMessage" }],
            "condition": { "type": "IsBlank" }
        }))
        .is_ok()
    );
}

// ---- saving (US-C2) ----

// US-C2
#[test]
fn test_normalize_and_validate_rejects_a_rule_without_actions() {
    for mut rule in [message_rule(vec![], words("spam")), join_rule(vec![], None)] {
        let err = rule.normalize_and_validate().unwrap_err().to_string();
        assert!(err.contains("A rule has no actions"), "{err}");
    }
}

// US-F4: a saved rule's actions are merged and ordered.
#[test]
fn test_normalize_and_validate_canonicalizes_the_action_list() {
    for mut rule in [message_rule(
        vec![kick(), moderate(), observer(0), observer(0)],
        words("spam"),
    )] {
        rule.normalize_and_validate().unwrap();
        assert_eq!(RuleDraft::from(rule).actions, vec![moderate(), kick()]);
    }
    let mut join = join_rule(vec![kick(), observer(0), observer(0)], None);
    join.normalize_and_validate().unwrap();
    assert_eq!(RuleDraft::from(join).actions, vec![kick()]);
}

// US-C2: the name is at most 60 characters, counted as characters.
#[test]
fn test_a_name_longer_than_60_characters_is_refused() {
    let mut longest = message_rule(vec![moderate()], words("spam"));
    longest.name = Some("я".repeat(60));
    longest.normalize_and_validate().unwrap();

    let mut rule = message_rule(vec![moderate()], words("spam"));
    rule.name = Some("я".repeat(61));
    let err = rule.normalize_and_validate().unwrap_err().to_string();
    assert!(
        err.contains("A rule's name is longer than 60 characters."),
        "{err}"
    );

    let mut join = join_rule(vec![kick()], None);
    join.name = Some("a".repeat(61));
    assert!(join.normalize_and_validate().is_err());
}

// US-G14: a blank name is no name, and a name is kept as typed.
#[test]
fn test_a_blank_name_is_no_name() {
    let mut rule = message_rule(vec![moderate()], words("spam"));
    rule.name = Some("   ".to_string());
    rule.normalize_and_validate().unwrap();
    assert_eq!(rule.name, None);

    let mut rule = message_rule(vec![moderate()], words("spam"));
    rule.name = Some("Banned words".to_string());
    rule.normalize_and_validate().unwrap();
    assert_eq!(rule.name.as_deref(), Some("Banned words"));
}

// US-G13: a rule that is off is checked like any other.
#[test]
fn test_a_rule_that_is_off_is_validated_too() {
    let mut rule = message_rule(vec![], words("spam"));
    rule.enabled = false;
    assert!(rule.normalize_and_validate().is_err());
}

// US-C2: the condition's own limits apply in a join rule.
#[test]
fn test_a_join_rules_condition_is_checked() {
    let mut rule = join_rule(
        vec![kick()],
        Some(ModerationCondition::MemberNameContainsWords(
            MemberNameContainsWords {
                keywords: vec!["x".repeat(101)],
            },
        )),
    );
    assert!(rule.normalize_and_validate().is_err());
}

// US-C4: the keys a rule carries are checked when it is saved.
#[test]
fn test_the_keys_a_rule_carries_are_listed() {
    let ai = ModerationCondition::FlaggedByOpenRouterInstruction(FlaggedByOpenRouterInstruction {
        api_key: "sk-or-key-a1b2".to_string(),
        model: "google/gemini-2.5-flash-lite".to_string(),
        instruction: "ads".to_string(),
        context_messages: 0,
        retry: Default::default(),
    });
    let rule = message_rule(vec![moderate()], ai);
    assert_eq!(rule.key_uses().len(), 1);
    let none = message_rule(vec![moderate()], words("spam"));
    assert!(none.key_uses().is_empty());
    let rate = message_rule(
        vec![moderate()],
        ModerationCondition::AuthorHitsMessageRateLimit(AuthorHitsMessageRateLimit {
            message_count: 3,
            time_window_minutes: 1,
        }),
    );
    assert!(rate.key_uses().is_empty());
}
