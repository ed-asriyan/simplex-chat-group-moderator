//! Checks an owner's settings pass when a rule is saved, shared by every
//! condition with a list or a count among its settings.

use crate::domain::moderator::ports::Err;

/// Maximum number of entries in a single condition's keyword / message / domain list.
const MAX_LIST_ENTRIES: usize = 10_000;

/// Drop blank entries and reduce the list to a sorted set. Blank entries are
/// dropped rather than rejected because the editor's list widget produces them
/// whenever a row is added and left untouched.
pub fn normalize_list(values: &mut Vec<String>) {
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
}

/// `noun` names the list in the "too many" message, `entry` a single entry in
/// the "too long" one, so both read naturally for keywords, messages and domains.
pub fn check_list(
    values: &[String],
    max_length: usize,
    noun: &str,
    entry: &str,
) -> Result<(), Err> {
    if values.len() > MAX_LIST_ENTRIES {
        return Err(format!(
            "Too many {noun}: {} provided, maximum is {MAX_LIST_ENTRIES}",
            values.len()
        )
        .into());
    }
    if let Some(value) = values.iter().find(|v| v.chars().count() > max_length) {
        return Err(format!(
            "{entry} too long: {} characters, maximum is {max_length}",
            value.chars().count()
        )
        .into());
    }
    Ok(())
}

/// Reject a zero where the condition could otherwise never match. The error
/// names the condition as the editor titles it, so the owner can find it.
pub fn check_nonzero(value: u32, error: &str) -> Result<(), Err> {
    if value == 0 {
        return Err(error.into());
    }
    Ok(())
}
