//! The rules' JSON, frozen.
//!
//! The same JSON travels in the editor link and comes back from the owner, so
//! the shape of every condition and action is a contract with the web editor
//! and with every link already sent. These tests pin it byte for byte, so a
//! refactor of the Rust types cannot change it unnoticed.

use super::{ModerationAction, ModerationCondition, ModerationRule};
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;

/// Every condition and action type with a non-default value in every field,
/// exactly as the bot serializes it today.
const EVERY_TYPE: &str = include_str!("format_tests/every_type.json");

#[test]
fn test_every_type_round_trips_unchanged() {
    let rules: Vec<ModerationRule> = serde_json::from_str(EVERY_TYPE).unwrap();
    assert_eq!(
        serde_json::to_string_pretty(&rules).unwrap(),
        EVERY_TYPE.trim_end()
    );
}

/// The variant names serde knows for `T`, read from the error it gives for an
/// unknown tag. Asking serde rather than listing them here is what makes a
/// new variant fail [`test_fixture_covers_every_type`] until it is added to
/// the fixture.
fn every_tag<T: DeserializeOwned>() -> BTreeSet<String> {
    let Err(error) = serde_json::from_str::<T>(r#"{"type":"\u0000"}"#) else {
        panic!("an unknown tag deserialized");
    };
    let message = error.to_string();
    let (_, expected) = message
        .split_once("expected one of ")
        .expect("serde names the variants it expects");
    expected
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The `type` of every object under `key` in `value`, at any depth.
fn tags_under(value: &serde_json::Value, key: &str, found: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (name, child) in map {
                if name == key {
                    collect_tags(child, found);
                }
                tags_under(child, key, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                tags_under(item, key, found);
            }
        }
        _ => {}
    }
}

fn collect_tags(value: &serde_json::Value, found: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_tags(item, found);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(tag)) = map.get("type") {
                found.insert(tag.clone());
            }
        }
        _ => {}
    }
}

#[test]
fn test_fixture_covers_every_type() {
    let fixture: serde_json::Value = serde_json::from_str(EVERY_TYPE).unwrap();

    let mut conditions = BTreeSet::new();
    tags_under(&fixture, "condition", &mut conditions);
    tags_under(&fixture, "conditions", &mut conditions);
    assert_eq!(conditions, every_tag::<ModerationCondition>());

    let mut actions = BTreeSet::new();
    tags_under(&fixture, "actions", &mut actions);
    assert_eq!(actions, every_tag::<ModerationAction>());
}
