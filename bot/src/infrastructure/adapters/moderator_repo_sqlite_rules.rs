use crate::domain::moderator::ports::{Err, ModerationRule, OwnedModerationRule};
use rusqlite::params;
use std::sync::{Arc, Mutex};

pub(crate) fn load_rules_for_group(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    gid: i64,
) -> Result<Vec<OwnedModerationRule>, Err> {
    let guard = conn.lock().unwrap();
    let mut rules = Vec::new();

    let mut stmt =
        guard.prepare("SELECT id, rule_type FROM moderation_rules WHERE group_id = ?1")?;
    let rule_rows = stmt.query_map(params![gid], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rule_rows {
        let (rule_id, rule_type) = row?;

        if rule_type == "words_blacklist" {
            let mut kw_stmt =
                guard.prepare("SELECT keyword FROM moderation_rule_keywords WHERE rule_id = ?1")?;
            let kw_rows = kw_stmt.query_map(params![rule_id], |row| row.get::<_, String>(0))?;
            let mut keywords = Vec::new();
            for kw in kw_rows {
                keywords.push(kw?);
            }
            rules.push(OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule::WordsBlacklist { keywords },
            });
        } else if rule_type == "links_whitelist_top100" {
            rules.push(OwnedModerationRule {
                id: rule_id as usize,
                rule: ModerationRule::LinksWhitelistTop100 {},
            });
        } else if rule_type == "links_blacklist" || rule_type == "links_whitelist" {
            let mut dom_stmt = guard
                .prepare("SELECT domain FROM moderation_rule_link_domains WHERE rule_id = ?1")?;
            let dom_rows = dom_stmt.query_map(params![rule_id], |row| row.get::<_, String>(0))?;
            let mut domains = Vec::new();
            for dom in dom_rows {
                domains.push(dom?);
            }
            if rule_type == "links_blacklist" {
                rules.push(OwnedModerationRule {
                    id: rule_id as usize,
                    rule: ModerationRule::LinksBlacklist { blocked: domains },
                });
            } else {
                rules.push(OwnedModerationRule {
                    id: rule_id as usize,
                    rule: ModerationRule::LinksWhitelist { allowed: domains },
                });
            }
        }
    }
    Ok(rules)
}
