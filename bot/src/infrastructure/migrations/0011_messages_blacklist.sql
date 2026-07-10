CREATE TABLE moderation_rule__messages_blacklist (
    id             INTEGER PRIMARY KEY,
    group_id       INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank           INTEGER NOT NULL,
    case_sensitive BOOLEAN NOT NULL DEFAULT 0
);
CREATE INDEX idx_moderation_rule__messages_blacklist_group_id
    ON moderation_rule__messages_blacklist (group_id);

CREATE TABLE moderation_rule__messages_blacklist__messages (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__messages_blacklist(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    PRIMARY KEY (rule_id, message)
);
