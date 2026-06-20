use crate::domain::moderator::ports::{Err, OwnedModerationRule, ModerationRule, ModerationRuleType};
use crate::domain::moderator::message_filter::ModerationRuleLink;
use rusqlite::params;
use std::sync::{Arc, Mutex};

pub(crate) fn load_rules_for_group(conn: &Arc<Mutex<rusqlite::Connection>>, gid: i64) -> Result<Vec<OwnedModerationRule>, Err> {
    let guard = conn.lock().unwrap();
    let mut rules = Vec::new();

    let mut stmt = guard.prepare("SELECT id, rule_type, enabled FROM moderation_rules WHERE group_id = ?1")?;
    let rule_rows = stmt.query_map(params![gid], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?))
    })?;

    for row in rule_rows {
        let (rule_id, rule_type, enabled) = row?;

        if rule_type == "keywords" {
            let mut kw_stmt = guard.prepare("SELECT keyword FROM moderation_rule_keywords WHERE rule_id = ?1")?;
            let kw_rows = kw_stmt.query_map(params![rule_id], |row| row.get::<_, String>(0))?;
            let mut keywords = Vec::new();
            for kw in kw_rows {
                keywords.push(kw?);
            }
            rules.push(OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule {
                    enabled,
                    rule_type: ModerationRuleType::Keywords(keywords),
                }
            });
        } else if rule_type == "link" {
            let mut ln_stmt = guard.prepare("SELECT inclusive, allow_top100 FROM moderation_rule_links WHERE rule_id = ?1")?;
            let mut ln_iter = ln_stmt.query_map(params![rule_id], |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)))?;
            
            if let Some(Ok((inclusive, allow_top100))) = ln_iter.next() {
                let mut allowed = Vec::new();
                let mut blocked = Vec::new();
                let mut dom_stmt = guard.prepare("SELECT domain, is_allowed FROM moderation_rule_link_domains WHERE rule_id = ?1")?;
                let dom_rows = dom_stmt.query_map(params![rule_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)))?;
                for dom in dom_rows {
                    let (domain, is_allowed) = dom?;
                    if is_allowed {
                        allowed.push(domain);
                    } else {
                        blocked.push(domain);
                    }
                }
                rules.push(OwnedModerationRule {
                    id: rule_id as usize,
                    rule: ModerationRule {
                        enabled,
                        rule_type: ModerationRuleType::Link(ModerationRuleLink {
                            inclusive,
                            allowed,
                            blocked,
                            allow_top100,
                        })
                    }
                });
            }
        }
    }

    Ok(rules)
}
