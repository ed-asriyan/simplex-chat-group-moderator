CREATE TABLE moderation_rule__links_whitelist_top100__allowed (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__links_whitelist_top100(id) ON DELETE CASCADE,
    domain  TEXT NOT NULL,
    PRIMARY KEY (rule_id, domain)
);
