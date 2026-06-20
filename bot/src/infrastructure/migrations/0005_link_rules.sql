CREATE TABLE IF NOT EXISTS moderation_group_link_rules (
    group_id INTEGER PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    inclusive BOOLEAN NOT NULL DEFAULT 0,
    allow_top100 BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY (group_id) REFERENCES moderation_groups (group_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS moderation_group_link_rule_domains (
    group_id INTEGER NOT NULL,
    domain TEXT NOT NULL,
    is_allowed BOOLEAN NOT NULL,
    PRIMARY KEY (group_id, domain),
    FOREIGN KEY (group_id) REFERENCES moderation_group_link_rules (group_id) ON DELETE CASCADE
);