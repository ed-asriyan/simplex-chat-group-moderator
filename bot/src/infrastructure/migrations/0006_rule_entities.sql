CREATE TABLE moderation_rules (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1
);

CREATE TABLE moderation_rule_keywords (
    rule_id INTEGER NOT NULL REFERENCES moderation_rules(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,
    PRIMARY KEY (rule_id, keyword)
);

CREATE TABLE moderation_rule_links (
    rule_id INTEGER PRIMARY KEY REFERENCES moderation_rules(id) ON DELETE CASCADE,
    inclusive BOOLEAN NOT NULL DEFAULT 0,
    allow_top100 BOOLEAN NOT NULL DEFAULT 0
);

CREATE TABLE moderation_rule_link_domains (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule_links(rule_id) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    is_allowed BOOLEAN NOT NULL,
    PRIMARY KEY(rule_id, domain)
);

-- Migrate old keywords
-- For each group that HAS keywords, we need exactly one new rule.
INSERT INTO moderation_rules (group_id, rule_type, enabled)
SELECT DISTINCT group_id, 'keywords', 1 FROM moderation_group_keywords;

INSERT INTO moderation_rule_keywords (rule_id, keyword)
SELECT r.id, k.keyword
FROM moderation_group_keywords k
JOIN moderation_rules r ON r.group_id = k.group_id AND r.rule_type = 'keywords';

DROP TABLE moderation_group_keywords;

-- Migrate old links (if any existed from migration 0005)
INSERT INTO moderation_rules (group_id, rule_type, enabled)
SELECT group_id, 'link', enabled FROM moderation_group_link_rules;

INSERT INTO moderation_rule_links (rule_id, inclusive, allow_top100)
SELECT r.id, l.inclusive, l.allow_top100
FROM moderation_group_link_rules l
JOIN moderation_rules r ON r.group_id = l.group_id AND r.rule_type = 'link';

INSERT INTO moderation_rule_link_domains (rule_id, domain, is_allowed)
SELECT r.id, d.domain, d.is_allowed
FROM moderation_group_link_rule_domains d
JOIN moderation_rules r ON r.group_id = d.group_id AND r.rule_type = 'link';

DROP TABLE moderation_group_link_rule_domains;
DROP TABLE moderation_group_link_rules;
