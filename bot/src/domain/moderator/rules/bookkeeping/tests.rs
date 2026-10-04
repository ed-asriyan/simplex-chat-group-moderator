use super::*;
use crate::domain::moderator::ports::actions::ModerateMessage;
use crate::domain::moderator::ports::conditions::{
    AuthorHitsLineRateLimit, AuthorHitsMessageRateLimit, ContainsWords,
};
use crate::domain::moderator::ports::{ModerationAction, ModerationCondition};

fn rule(condition: ModerationCondition) -> ModerationRule {
    ModerationRule {
        actions: vec![ModerationAction::ModerateMessage(ModerateMessage {})],
        condition,
    }
}

#[test]
fn test_needs_are_merged_across_rules() {
    let rules = [
        rule(ModerationCondition::AuthorHitsMessageRateLimit(
            AuthorHitsMessageRateLimit {
                message_count: 3,
                time_window_minutes: 7,
            },
        )),
        rule(ModerationCondition::Not {
            condition: Box::new(ModerationCondition::AuthorHitsMessageRateLimit(
                AuthorHitsMessageRateLimit {
                    message_count: 9,
                    time_window_minutes: 30,
                },
            )),
        }),
        rule(ModerationCondition::AuthorHitsLineRateLimit(
            AuthorHitsLineRateLimit {
                line_count: 10,
                time_window_minutes: 5,
                chars_per_line: 40,
            },
        )),
    ];
    let needs = needs_of(&rules);
    assert_eq!(needs.author_messages, Some(30));
    assert_eq!(needs.author_lines.map(|l| l.chars_per_line), Some(40));
    assert_eq!(needs.author_characters, None);
}

#[test]
fn test_rules_that_read_no_counter_need_nothing() {
    let rules = [rule(ModerationCondition::ContainsWords(ContainsWords {
        keywords: vec!["spam".to_string()],
    }))];
    assert_eq!(needs_of(&rules), Needs::default());
}

#[test]
fn test_counters_keep_nothing_past_an_hour() {
    assert_eq!(ttl(5), Duration::from_secs(5 * 60));
    assert_eq!(ttl(600), Duration::from_secs(60 * 60));
}

#[test]
fn test_an_attachment_without_a_caption_adds_no_lines() {
    assert_eq!(lines("", 40), 0);
    assert_eq!(lines("one\ntwo", 0), 2);
}
