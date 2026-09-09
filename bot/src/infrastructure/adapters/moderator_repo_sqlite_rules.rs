use crate::domain::moderator::ports::{
    DeleteAuthorMessages, DeleteObserverMessages, Err, ModerationAction, ModerationRule,
    OwnedModerationRule, RuleCondition,
};
use rusqlite::params;
use std::sync::{Arc, Mutex};

/// Resolve a rule's action from the `moderation_actions` registry (plus its
/// per-type subtable). `None` means the rule carries no action row and falls
/// back to the default of moderating the message.
fn load_action(
    guard: &rusqlite::Connection,
    action_id: Option<i64>,
) -> Result<ModerationAction, Err> {
    let Some(action_id) = action_id else {
        return Ok(ModerationAction::ModerateMessage);
    };
    let action_type: String = guard.query_row(
        "SELECT type FROM moderation_actions WHERE id = ?1",
        params![action_id],
        |row| row.get(0),
    )?;
    match action_type.as_str() {
        "KickAuthor" => {
            let delete_messages_code: i64 = guard.query_row(
                "SELECT delete_messages FROM moderation_action__kick_author WHERE action_id = ?1",
                params![action_id],
                |row| row.get(0),
            )?;
            let delete_messages = match delete_messages_code {
                0 => DeleteAuthorMessages::None,
                2 => DeleteAuthorMessages::AllMessages,
                _ => DeleteAuthorMessages::TriggeredMessage,
            };
            Ok(ModerationAction::KickAuthor { delete_messages })
        }
        "SetAuthorObserver" => {
            let delete_message_code: i64 = guard.query_row(
                "SELECT delete_message FROM moderation_action__set_author_observer WHERE action_id = ?1",
                params![action_id],
                |row| row.get(0),
            )?;
            let delete_message = match delete_message_code {
                0 => DeleteObserverMessages::None,
                _ => DeleteObserverMessages::TriggeredMessage,
            };
            Ok(ModerationAction::SetAuthorObserver { delete_message })
        }
        _ => Ok(ModerationAction::ModerateMessage),
    }
}

/// Load the child rows (keywords/domains) for a rule from a subtable keyed by
/// `rule_id`, returning them in `id` order so the result is deterministic.
fn load_rule_values(
    guard: &rusqlite::Connection,
    table: &str,
    column: &str,
    rule_id: i64,
) -> Result<Vec<String>, Err> {
    let sql = format!("SELECT {column} FROM {table} WHERE rule_id = ?1");
    let mut stmt = guard.prepare(&sql)?;
    let rows = stmt.query_map(params![rule_id], |row| row.get::<_, String>(0))?;
    let mut values = Vec::new();
    for value in rows {
        values.push(value?);
    }
    Ok(values)
}

pub(crate) fn load_rules_for_group(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    gid: i64,
) -> Result<Vec<OwnedModerationRule>, Err> {
    let guard = conn.lock().expect("moderation repo connection poisoned");
    // Collect each rule alongside its global `rank` so the original
    // editor-supplied order can be reconstructed across the split tables.
    let mut ranked: Vec<(i64, OwnedModerationRule)> = Vec::new();

    // WordsBlacklist
    let mut stmt = guard.prepare(
        "SELECT id, rank, action_id FROM moderation_rule__words_blacklist WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map(params![gid], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank, action_id) in rows {
        let keywords = load_rule_values(
            &guard,
            "moderation_rule__words_blacklist__keywords",
            "keyword",
            rule_id,
        )?;
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::WordsBlacklist { keywords },
                },
            },
        ));
    }

    // MessagesBlacklist
    let mut stmt = guard.prepare(
        "SELECT id, rank, case_sensitive, action_id FROM moderation_rule__messages_blacklist WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, bool, Option<i64>)> = stmt
        .query_map(params![gid], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank, case_sensitive, action_id) in rows {
        let messages = load_rule_values(
            &guard,
            "moderation_rule__messages_blacklist__messages",
            "message",
            rule_id,
        )?;
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::MessagesBlacklist {
                        messages,
                        case_sensitive,
                    },
                },
            },
        ));
    }

    // LinksBlacklist
    let mut stmt = guard.prepare(
        "SELECT id, rank, action_id FROM moderation_rule__links_blacklist WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map(params![gid], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank, action_id) in rows {
        let blocked = load_rule_values(
            &guard,
            "moderation_rule__links_blacklist__domains",
            "domain",
            rule_id,
        )?;
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::LinksBlacklist { blocked },
                },
            },
        ));
    }

    // LinksWhitelist
    let mut stmt = guard.prepare(
        "SELECT id, rank, action_id FROM moderation_rule__links_whitelist WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map(params![gid], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank, action_id) in rows {
        let allowed = load_rule_values(
            &guard,
            "moderation_rule__links_whitelist__domains",
            "domain",
            rule_id,
        )?;
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::LinksWhitelist { allowed },
                },
            },
        ));
    }

    // LinksWhitelistTop100
    let mut stmt = guard.prepare(
        "SELECT id, rank, action_id FROM moderation_rule__links_whitelist_top100 WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map(params![gid], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank, action_id) in rows {
        let allowed = load_rule_values(
            &guard,
            "moderation_rule__links_whitelist_top100__allowed",
            "domain",
            rule_id,
        )?;
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::LinksWhitelistTop100 { allowed },
                },
            },
        ));
    }

    // ScreenFlooding
    let mut stmt = guard.prepare(
        "SELECT id, rank, max_characters, max_words, max_lines, chars_per_line, disallow_invisible_chars, disallow_empty_messages, action_id FROM moderation_rule__screen_flooding WHERE group_id = ?1",
    )?;
    let rows: Vec<(
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        bool,
        bool,
        Option<i64>,
    )> = stmt
        .query_map(params![gid], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    for (
        rule_id,
        rank,
        max_characters,
        max_words,
        max_lines,
        chars_per_line,
        disallow_invisible_chars,
        disallow_empty_messages,
        action_id,
    ) in rows
    {
        ranked.push((
            rank,
            OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    action: load_action(&guard, action_id)?,
                    condition: RuleCondition::ScreenFlooding {
                        max_characters: max_characters.unwrap_or(0) as u32,
                        max_words: max_words.unwrap_or(0) as u32,
                        max_lines: max_lines.unwrap_or(0) as u32,
                        chars_per_line: chars_per_line.unwrap_or(40) as u32,
                        disallow_invisible_chars,
                        disallow_empty_messages,
                    },
                },
            },
        ));
    }

    // Restore the original (editor) order. Ties (same rank) fall back to id for
    // a deterministic result.
    ranked.sort_by_key(|(rank, owned)| (*rank, owned.id));
    Ok(ranked.into_iter().map(|(_, owned)| owned).collect())
}
