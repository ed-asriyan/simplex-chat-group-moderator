-- Normalize the per-rule "action" out of the individual rule tables.
--
-- `moderation_actions` is a thin registry: one row per action instance, whose
-- `type` names which concrete action it is. Per-type settings live in a subtype
-- detail table `moderation_action__<name>` keyed by `action_id` (class-table
-- inheritance). Each `moderation_rule__*` row points at its action through a
-- real `action_id` foreign key into this single registry, so the reference is
-- enforceable (and portable to a server RDBMS) instead of being an ambiguous
-- "id into one of several tables".

CREATE TABLE moderation_actions (
    id   INTEGER PRIMARY KEY,
    type TEXT NOT NULL
);

-- ModerateMessage: no settings.
CREATE TABLE moderation_action__moderate_message (
    action_id INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE
);

-- KickAuthor: specifies what to do with the author's messages:
-- 0: None (do not delete)
-- 1: TriggeredMessage (delete offending message)
-- 2: AllMessages (delete all author's messages)
CREATE TABLE moderation_action__kick_author (
    action_id       INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE,
    delete_messages INTEGER NOT NULL
);

-- Link every rule to its action via a real foreign key. The column is nullable
-- only so it can be added to the existing tables; the backfill below populates
-- every current row and the application always writes it going forward.
ALTER TABLE moderation_rule__words_blacklist        ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);
ALTER TABLE moderation_rule__messages_blacklist     ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);
ALTER TABLE moderation_rule__links_blacklist        ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);
ALTER TABLE moderation_rule__links_whitelist        ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);
ALTER TABLE moderation_rule__links_whitelist_top100 ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);
ALTER TABLE moderation_rule__screen_flooding        ADD COLUMN action_id INTEGER REFERENCES moderation_actions(id);

-- Backfill: every existing rule keeps its prior behaviour, which was always
-- "moderate the message". Create one ModerateMessage action per rule and link
-- it. Explicit ids (assigned via ROW_NUMBER) make the rule <-> action mapping
-- deterministic instead of relying on insert-order rowid assignment.
CREATE TEMP TABLE _rules_needing_action (
    tbl     TEXT    NOT NULL,
    rule_id INTEGER NOT NULL
);
INSERT INTO _rules_needing_action (tbl, rule_id)
    SELECT 'words_blacklist', id FROM moderation_rule__words_blacklist
    UNION ALL SELECT 'messages_blacklist', id FROM moderation_rule__messages_blacklist
    UNION ALL SELECT 'links_blacklist', id FROM moderation_rule__links_blacklist
    UNION ALL SELECT 'links_whitelist', id FROM moderation_rule__links_whitelist
    UNION ALL SELECT 'links_whitelist_top100', id FROM moderation_rule__links_whitelist_top100
    UNION ALL SELECT 'screen_flooding', id FROM moderation_rule__screen_flooding;

CREATE TEMP TABLE _action_map (
    tbl       TEXT    NOT NULL,
    rule_id   INTEGER NOT NULL,
    action_id INTEGER NOT NULL
);
INSERT INTO _action_map (tbl, rule_id, action_id)
    SELECT tbl, rule_id, ROW_NUMBER() OVER (ORDER BY tbl, rule_id)
    FROM _rules_needing_action;

INSERT INTO moderation_actions (id, type)
    SELECT action_id, 'ModerateMessage' FROM _action_map;
INSERT INTO moderation_action__moderate_message (action_id)
    SELECT action_id FROM _action_map;

UPDATE moderation_rule__words_blacklist
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'words_blacklist' AND rule_id = moderation_rule__words_blacklist.id);
UPDATE moderation_rule__messages_blacklist
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'messages_blacklist' AND rule_id = moderation_rule__messages_blacklist.id);
UPDATE moderation_rule__links_blacklist
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'links_blacklist' AND rule_id = moderation_rule__links_blacklist.id);
UPDATE moderation_rule__links_whitelist
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'links_whitelist' AND rule_id = moderation_rule__links_whitelist.id);
UPDATE moderation_rule__links_whitelist_top100
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'links_whitelist_top100' AND rule_id = moderation_rule__links_whitelist_top100.id);
UPDATE moderation_rule__screen_flooding
    SET action_id = (SELECT action_id FROM _action_map
                     WHERE tbl = 'screen_flooding' AND rule_id = moderation_rule__screen_flooding.id);

DROP TABLE _action_map;
DROP TABLE _rules_needing_action;
