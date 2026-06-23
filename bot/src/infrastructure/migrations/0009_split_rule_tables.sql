-- Replace the generic `moderation_rules` registry (id, group_id, rule_type)
-- plus the shared `moderation_rule_keywords` / `moderation_rule_link_domains`
-- subtables with one typed table per rule type. Each typed table owns its
-- per-rule settings columns (none yet besides the lists, but this is where they
-- will live), keeps its own surrogate `id`, and references `moderation_groups`
-- directly. Rule ids are preserved during backfill so the rule_id -> child-row
-- relationships carry over unchanged.

-- WordsBlacklist
CREATE TABLE moderation_rule__words_blacklist (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rule__words_blacklist_group_id
    ON moderation_rule__words_blacklist (group_id);

CREATE TABLE moderation_rule__words_blacklist__keywords (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__words_blacklist(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,
    PRIMARY KEY (rule_id, keyword)
);

-- LinksBlacklist
CREATE TABLE moderation_rule__links_blacklist (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rule__links_blacklist_group_id
    ON moderation_rule__links_blacklist (group_id);

CREATE TABLE moderation_rule__links_blacklist__domains (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__links_blacklist(id) ON DELETE CASCADE,
    domain  TEXT NOT NULL,
    PRIMARY KEY (rule_id, domain)
);

-- LinksWhitelist
CREATE TABLE moderation_rule__links_whitelist (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rule__links_whitelist_group_id
    ON moderation_rule__links_whitelist (group_id);

CREATE TABLE moderation_rule__links_whitelist__domains (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__links_whitelist(id) ON DELETE CASCADE,
    domain  TEXT NOT NULL,
    PRIMARY KEY (rule_id, domain)
);

-- LinksWhitelistTop100 (no per-rule data: the list is built into the binary)
CREATE TABLE moderation_rule__links_whitelist_top100 (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rule__links_whitelist_top100_group_id
    ON moderation_rule__links_whitelist_top100 (group_id);

-- Backfill from the old registry. Preserve ids so the existing
-- moderation_rule_keywords / moderation_rule_link_domains rows (keyed by
-- rule_id) line up with the new parents. The old registry had no explicit
-- ordering, so reuse the global `id` as `rank` to keep the prior (id-order)
-- presentation order stable across the now-split tables.
INSERT INTO moderation_rule__words_blacklist (id, group_id, rank)
    SELECT id, group_id, id FROM moderation_rules WHERE rule_type = 'words_blacklist';
INSERT INTO moderation_rule__words_blacklist__keywords (rule_id, keyword)
    SELECT k.rule_id, k.keyword
    FROM moderation_rule_keywords k
    JOIN moderation_rules r ON r.id = k.rule_id
    WHERE r.rule_type = 'words_blacklist';

INSERT INTO moderation_rule__links_blacklist (id, group_id, rank)
    SELECT id, group_id, id FROM moderation_rules WHERE rule_type = 'links_blacklist';
INSERT INTO moderation_rule__links_blacklist__domains (rule_id, domain)
    SELECT d.rule_id, d.domain
    FROM moderation_rule_link_domains d
    JOIN moderation_rules r ON r.id = d.rule_id
    WHERE r.rule_type = 'links_blacklist';

INSERT INTO moderation_rule__links_whitelist (id, group_id, rank)
    SELECT id, group_id, id FROM moderation_rules WHERE rule_type = 'links_whitelist';
INSERT INTO moderation_rule__links_whitelist__domains (rule_id, domain)
    SELECT d.rule_id, d.domain
    FROM moderation_rule_link_domains d
    JOIN moderation_rules r ON r.id = d.rule_id
    WHERE r.rule_type = 'links_whitelist';

INSERT INTO moderation_rule__links_whitelist_top100 (id, group_id, rank)
    SELECT id, group_id, id FROM moderation_rules WHERE rule_type = 'links_whitelist_top100';

-- Drop the old registry. Children first so foreign-key enforcement stays happy.
DROP TABLE moderation_rule_keywords;
DROP TABLE moderation_rule_link_domains;
DROP TABLE moderation_rules;
