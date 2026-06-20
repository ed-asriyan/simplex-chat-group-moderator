CREATE TABLE moderation_rules (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL
);

CREATE TABLE moderation_rule_keywords (
    rule_id INTEGER NOT NULL REFERENCES moderation_rules(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,
    PRIMARY KEY (rule_id, keyword)
);

CREATE TABLE moderation_rule_link_domains (
    rule_id INTEGER NOT NULL REFERENCES moderation_rules(id) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    PRIMARY KEY(rule_id, domain)
);

-- Note: Migration 0005 wasn't released so it has been fully absorbed here.
DROP TABLE IF EXISTS moderation_group_link_rule_domains;
DROP TABLE IF EXISTS moderation_group_link_rules;
