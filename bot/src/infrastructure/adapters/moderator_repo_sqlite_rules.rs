//! Reading a group's rules back out of SQLite.
//!
//! A rule is a row in `moderation_rules` plus a tree of `moderation_conditions`
//! rows plus one `moderation_actions` row, all of which point *up* at their
//! parent. Rather than walking the tree with a query per node, everything a
//! group owns is fetched with a fixed number of queries — one per table — and
//! the tree is assembled in memory. The query count is therefore independent of
//! how many rules the group has and how deeply nested they are.

use std::collections::HashMap;

use crate::domain::moderator::ports::{
    DeleteAuthorMessages, DeleteObserverMessages, Err, ModerationAction, ModerationCondition,
    ModerationRule, OwnedModerationRule,
};
use rusqlite::{OptionalExtension, params};
use std::sync::{Arc, Mutex};

/// Resolve a rule's action from the `moderation_actions` registry (plus its
/// per-type subtable). A rule with no action row falls back to the default of
/// moderating the message.
fn load_action(guard: &rusqlite::Connection, rule_id: i64) -> Result<ModerationAction, Err> {
    let row: Option<(i64, String)> = guard
        .query_row(
            "SELECT id, type FROM moderation_actions WHERE rule_id = ?1",
            params![rule_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((action_id, action_type)) = row else {
        return Ok(ModerationAction::ModerateMessage);
    };
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

/// Load a whole group's rows from a condition subtable (keywords, domains, ...)
/// grouped by condition id.
fn load_condition_lists(
    guard: &rusqlite::Connection,
    table: &str,
    column: &str,
    gid: i64,
) -> Result<HashMap<i64, Vec<String>>, Err> {
    let sql = format!(
        "SELECT s.condition_id, s.{column}
           FROM {table} s
           JOIN moderation_conditions c ON c.id = s.condition_id
           JOIN moderation_rules r ON r.id = c.rule_id
          WHERE r.group_id = ?1"
    );
    let mut stmt = guard.prepare(&sql)?;
    let rows = stmt.query_map(params![gid], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out: HashMap<i64, Vec<String>> = HashMap::new();
    for row in rows {
        let (condition_id, value) = row?;
        out.entry(condition_id).or_default().push(value);
    }
    Ok(out)
}

/// Load a whole group's rows from a condition settings table, mapping each
/// condition id to the settings tuple produced by `map_row`.
fn load_condition_settings<T, F>(
    guard: &rusqlite::Connection,
    columns: &str,
    table: &str,
    gid: i64,
    map_row: F,
) -> Result<HashMap<i64, T>, Err>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let sql = format!(
        "SELECT s.condition_id, {columns}
           FROM {table} s
           JOIN moderation_conditions c ON c.id = s.condition_id
           JOIN moderation_rules r ON r.id = c.rule_id
          WHERE r.group_id = ?1"
    );
    let mut stmt = guard.prepare(&sql)?;
    let rows = stmt.query_map(params![gid], |row| {
        Ok((row.get::<_, i64>(0)?, map_row(row)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (condition_id, value) = row?;
        out.insert(condition_id, value);
    }
    Ok(out)
}

/// Settings loaded for every condition node of one group, keyed by condition id.
struct ConditionData {
    banned_words: HashMap<i64, Vec<String>>,
    exact_messages: HashMap<i64, Vec<String>>,
    exact_message_settings: HashMap<i64, bool>,
    regex_patterns: HashMap<i64, Vec<String>>,
    repeated_sequence: HashMap<i64, (u32, u32)>,
    forbidden_domains: HashMap<i64, Vec<String>>,
    allowed_domains: HashMap<i64, Vec<String>>,
    top100_allowed: HashMap<i64, Vec<String>>,
    flooding: HashMap<i64, (u32, u32, u32, u32, bool, bool)>,
    messages_rate_limit: HashMap<i64, (u32, u32)>,
    moderation_rate_limit: HashMap<i64, (u32, u32)>,
    joined_recently: HashMap<i64, u32>,
}

impl ConditionData {
    fn load(guard: &rusqlite::Connection, gid: i64) -> Result<Self, Err> {
        Ok(Self {
            banned_words: load_condition_lists(
                guard,
                "moderation_condition__contains_banned_words__keywords",
                "keyword",
                gid,
            )?,
            exact_messages: load_condition_lists(
                guard,
                "moderation_condition__matches_exact_message__messages",
                "message",
                gid,
            )?,
            exact_message_settings: load_condition_settings(
                guard,
                "s.case_sensitive",
                "moderation_condition__matches_exact_message",
                gid,
                |row| row.get::<_, bool>(1),
            )?,
            regex_patterns: load_condition_lists(
                guard,
                "moderation_condition__matches_regex__patterns",
                "pattern",
                gid,
            )?,
            repeated_sequence: load_condition_settings(
                guard,
                "s.min_repeats, s.min_length",
                "moderation_condition__contains_repeated_sequence",
                gid,
                |row| Ok((row.get::<_, i64>(1)? as u32, row.get::<_, i64>(2)? as u32)),
            )?,
            forbidden_domains: load_condition_lists(
                guard,
                "moderation_condition__contains_links_to_forbidden_websites__domains",
                "domain",
                gid,
            )?,
            allowed_domains: load_condition_lists(
                guard,
                "moderation_condition__contains_links_outside_allowed_list__domains",
                "domain",
                gid,
            )?,
            top100_allowed: load_condition_lists(
                guard,
                "moderation_condition__contains_links_outside_top100__allowed",
                "domain",
                gid,
            )?,
            flooding: load_condition_settings(
                guard,
                "s.max_characters, s.max_words, s.max_lines, s.chars_per_line, \
                 s.disallow_invisible_chars, s.disallow_empty_messages",
                "moderation_condition__floods_chat_or_exceeds_limits",
                gid,
                |row| {
                    Ok((
                        row.get::<_, i64>(1)? as u32,
                        row.get::<_, i64>(2)? as u32,
                        row.get::<_, i64>(3)? as u32,
                        row.get::<_, i64>(4)? as u32,
                        row.get::<_, bool>(5)?,
                        row.get::<_, bool>(6)?,
                    ))
                },
            )?,
            messages_rate_limit: load_condition_settings(
                guard,
                "s.message_count, s.time_window_minutes",
                "moderation_condition__user_exceeds_messages_rate_limit",
                gid,
                |row| Ok((row.get::<_, i64>(1)? as u32, row.get::<_, i64>(2)? as u32)),
            )?,
            moderation_rate_limit: load_condition_settings(
                guard,
                "s.message_count, s.time_window_minutes",
                "moderation_condition__user_exceeds_moderation_rate_limit",
                gid,
                |row| Ok((row.get::<_, i64>(1)? as u32, row.get::<_, i64>(2)? as u32)),
            )?,
            joined_recently: load_condition_settings(
                guard,
                "s.time_window_minutes",
                "moderation_condition__user_joined_recently",
                gid,
                |row| Ok(row.get::<_, i64>(1)? as u32),
            )?,
        })
    }
}

/// Rebuild the condition rooted at `id` from the loaded rows.
fn build_condition(
    id: i64,
    nodes: &HashMap<i64, String>,
    children: &HashMap<i64, Vec<i64>>,
    data: &ConditionData,
) -> Result<ModerationCondition, Err> {
    let type_tag = nodes
        .get(&id)
        .ok_or_else(|| -> Err { format!("condition {id} is missing").into() })?;

    let build_children = || -> Result<Vec<ModerationCondition>, Err> {
        children
            .get(&id)
            .map(|ids| {
                ids.iter()
                    .map(|child| build_condition(*child, nodes, children, data))
                    .collect::<Result<Vec<_>, Err>>()
            })
            .unwrap_or_else(|| Ok(Vec::new()))
    };

    match type_tag.as_str() {
        "All" => Ok(ModerationCondition::All {
            conditions: build_children()?,
        }),
        "Any" => Ok(ModerationCondition::Any {
            conditions: build_children()?,
        }),
        "Not" => {
            let mut built = build_children()?;
            if built.len() != 1 {
                return Err(format!(
                    "condition {id} is a 'Not' with {} children, expected exactly 1",
                    built.len()
                )
                .into());
            }
            Ok(ModerationCondition::Not {
                condition: Box::new(built.remove(0)),
            })
        }
        "ContainsBannedWords" => Ok(ModerationCondition::ContainsBannedWords {
            keywords: data.banned_words.get(&id).cloned().unwrap_or_default(),
        }),
        "MatchesExactMessage" => Ok(ModerationCondition::MatchesExactMessage {
            messages: data.exact_messages.get(&id).cloned().unwrap_or_default(),
            case_sensitive: data
                .exact_message_settings
                .get(&id)
                .copied()
                .unwrap_or(false),
        }),
        "MatchesRegex" => Ok(ModerationCondition::MatchesRegex {
            patterns: data.regex_patterns.get(&id).cloned().unwrap_or_default(),
        }),
        "ContainsRepeatedSequence" => {
            let (min_repeats, min_length) =
                data.repeated_sequence.get(&id).copied().unwrap_or((0, 0));
            Ok(ModerationCondition::ContainsRepeatedSequence {
                min_repeats,
                min_length,
            })
        }
        "ContainsLinksToForbiddenWebsites" => {
            Ok(ModerationCondition::ContainsLinksToForbiddenWebsites {
                blocked: data.forbidden_domains.get(&id).cloned().unwrap_or_default(),
            })
        }
        "ContainsLinksOutsideAllowedList" => {
            Ok(ModerationCondition::ContainsLinksOutsideAllowedList {
                allowed: data.allowed_domains.get(&id).cloned().unwrap_or_default(),
            })
        }
        "ContainsLinksOutsideTop100" => Ok(ModerationCondition::ContainsLinksOutsideTop100 {
            allowed: data.top100_allowed.get(&id).cloned().unwrap_or_default(),
        }),
        "FloodsChatOrExceedsLimits" => {
            let (
                max_characters,
                max_words,
                max_lines,
                chars_per_line,
                disallow_invisible_chars,
                disallow_empty_messages,
            ) = data
                .flooding
                .get(&id)
                .copied()
                .unwrap_or((0, 0, 0, 40, false, true));
            Ok(ModerationCondition::FloodsChatOrExceedsLimits {
                max_characters,
                max_words,
                max_lines,
                chars_per_line,
                disallow_invisible_chars,
                disallow_empty_messages,
            })
        }
        "UserExceedsMessagesRateLimit" => {
            let (message_count, time_window_minutes) =
                data.messages_rate_limit.get(&id).copied().unwrap_or((0, 0));
            Ok(ModerationCondition::UserExceedsMessagesRateLimit {
                message_count,
                time_window_minutes,
            })
        }
        "UserExceedsModerationRateLimit" => {
            let (message_count, time_window_minutes) = data
                .moderation_rate_limit
                .get(&id)
                .copied()
                .unwrap_or((0, 0));
            Ok(ModerationCondition::UserExceedsModerationRateLimit {
                message_count,
                time_window_minutes,
            })
        }
        "UserJoinedRecently" => Ok(ModerationCondition::UserJoinedRecently {
            time_window_minutes: data.joined_recently.get(&id).copied().unwrap_or(0),
        }),
        other => Err(format!("unknown condition type '{other}' on condition {id}").into()),
    }
}

pub(crate) fn load_rules_for_group(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    gid: i64,
) -> Result<Vec<OwnedModerationRule>, Err> {
    let guard = conn.lock().expect("moderation repo connection poisoned");

    let mut stmt =
        guard.prepare("SELECT id FROM moderation_rules WHERE group_id = ?1 ORDER BY rank, id")?;
    let rule_ids: Vec<i64> = stmt
        .query_map(params![gid], |row| row.get::<_, i64>(0))?
        .collect::<Result<_, _>>()?;
    if rule_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Every condition node the group owns, in one query.
    let mut stmt = guard.prepare(
        "SELECT c.id, c.rule_id, c.parent_id, c.rank, c.type
           FROM moderation_conditions c
           JOIN moderation_rules r ON r.id = c.rule_id
          WHERE r.group_id = ?1",
    )?;
    let rows: Vec<(i64, i64, Option<i64>, i64, String)> = stmt
        .query_map(params![gid], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut nodes: HashMap<i64, String> = HashMap::new();
    let mut roots: HashMap<i64, i64> = HashMap::new();
    // (rank, id) pairs so siblings can be ordered before the ids are kept.
    let mut children_ranked: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    for (id, rule_id, parent_id, rank, type_tag) in rows {
        match parent_id {
            Some(parent) => children_ranked.entry(parent).or_default().push((rank, id)),
            None => {
                roots.insert(rule_id, id);
            }
        }
        nodes.insert(id, type_tag);
    }
    let children: HashMap<i64, Vec<i64>> = children_ranked
        .into_iter()
        .map(|(parent, mut ids)| {
            ids.sort_unstable();
            (parent, ids.into_iter().map(|(_, id)| id).collect())
        })
        .collect();

    let data = ConditionData::load(&guard, gid)?;

    let mut rules = Vec::with_capacity(rule_ids.len());
    for rule_id in rule_ids {
        let Some(root) = roots.get(&rule_id) else {
            // A rule with no condition cannot be evaluated and cannot be shown
            // back to the owner. Skip it rather than failing every other rule.
            log::warn!("rule {rule_id} has no root condition, skipping it");
            continue;
        };
        rules.push(OwnedModerationRule {
            id: rule_id as usize,
            rule: ModerationRule {
                action: load_action(&guard, rule_id)?,
                condition: build_condition(*root, &nodes, &children, &data)?,
            },
        });
    }
    Ok(rules)
}
