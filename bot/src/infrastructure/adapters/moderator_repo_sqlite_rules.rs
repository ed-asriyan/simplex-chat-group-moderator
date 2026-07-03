use crate::domain::moderator::ports::{Err, ModerationRule, OwnedModerationRule};
use rusqlite::params;
use std::sync::{Arc, Mutex};

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
    let mut stmt = guard
        .prepare("SELECT id, rank FROM moderation_rule__words_blacklist WHERE group_id = ?1")?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map(params![gid], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank) in rows {
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
                rule: ModerationRule::WordsBlacklist { keywords },
            },
        ));
    }

    // LinksBlacklist
    let mut stmt = guard
        .prepare("SELECT id, rank FROM moderation_rule__links_blacklist WHERE group_id = ?1")?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map(params![gid], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank) in rows {
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
                rule: ModerationRule::LinksBlacklist { blocked },
            },
        ));
    }

    // LinksWhitelist
    let mut stmt = guard
        .prepare("SELECT id, rank FROM moderation_rule__links_whitelist WHERE group_id = ?1")?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map(params![gid], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank) in rows {
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
                rule: ModerationRule::LinksWhitelist { allowed },
            },
        ));
    }

    // LinksWhitelistTop100
    let mut stmt = guard.prepare(
        "SELECT id, rank FROM moderation_rule__links_whitelist_top100 WHERE group_id = ?1",
    )?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map(params![gid], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (rule_id, rank) in rows {
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
                rule: ModerationRule::LinksWhitelistTop100 { allowed },
            },
        ));
    }

    // Restore the original (editor) order. Ties (same rank) fall back to id for
    // a deterministic result.
    ranked.sort_by_key(|(rank, owned)| (*rank, owned.id));
    Ok(ranked.into_iter().map(|(_, owned)| owned).collect())
}
