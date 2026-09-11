-- Turn a rule's condition from a single typed row into a tree of condition
-- nodes, and make the whole schema one tree rooted at `moderation_groups`.
--
-- Before: a rule *was* a row in `moderation_rule__<name>`, carrying group_id,
-- rank and a nullable action_id. That shape could hold exactly one condition
-- per rule, put the same three columns on every condition table, and pointed at
-- `moderation_actions` the wrong way round, so deleting a group left its action
-- rows behind for the application to sweep up by hand.
--
-- After:
--   moderation_groups
--     └── moderation_rules            (group_id, rank)
--           ├── moderation_conditions (rule_id, parent_id, rank, type)
--           │     └── moderation_condition__<name>[__<subtable>] (condition_id)
--           └── moderation_actions    (rule_id, type)
--                 └── moderation_action__<name> (action_id)
--
-- Every arrow is a real foreign key pointing at its parent with ON DELETE
-- CASCADE, so `DELETE FROM moderation_groups WHERE group_id = ?` removes every
-- row that belongs to the group and nothing needs to be deleted by hand.
--
-- Composite conditions (All / Any / Not) have no settings of their own, so they
-- get no `moderation_condition__<name>` table at all: the discriminator in
-- `moderation_conditions.type` already carries everything they need. This is a
-- deliberate departure from `moderation_actions`, where a settings-free action
-- still gets a table to keep the reference uniform — here the registry row is
-- the uniform part.

-- ---------------------------------------------------------------------------
-- Structure
-- ---------------------------------------------------------------------------

CREATE TABLE moderation_rules (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rules_group_id ON moderation_rules (group_id);

-- `rule_id` is denormalized onto every node, not just the root: it makes the
-- whole tree of a rule reachable with one indexed query instead of a recursive
-- descent, and it makes deletion flat (the cascade from the rule reaches every
-- node at once, whatever the depth). The writer maintains the invariant that a
-- node's rule_id equals its parent's.
--
-- The root of a rule's tree is its node with `parent_id IS NULL`. Pointing the
-- rule at its root instead would create a reference cycle between the two
-- tables and a second cascade path into the same rows.
CREATE TABLE moderation_conditions (
    id        INTEGER PRIMARY KEY,
    rule_id   INTEGER NOT NULL REFERENCES moderation_rules(id) ON DELETE CASCADE,
    parent_id INTEGER          REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    rank      INTEGER NOT NULL,
    type      TEXT    NOT NULL
);
CREATE INDEX idx_moderation_conditions_rule_id   ON moderation_conditions (rule_id);
CREATE INDEX idx_moderation_conditions_parent_id ON moderation_conditions (parent_id);

-- Per-condition settings. A condition whose only parameter is a list gets no
-- settings table, just the list subtable.

CREATE TABLE moderation_condition__contains_banned_words__keywords (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    keyword      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, keyword)
);

CREATE TABLE moderation_condition__matches_exact_message (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    case_sensitive BOOLEAN NOT NULL DEFAULT 0
);
CREATE TABLE moderation_condition__matches_exact_message__messages (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, message)
);

CREATE TABLE moderation_condition__matches_regex__patterns (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    pattern      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, pattern)
);

CREATE TABLE moderation_condition__contains_links_to_forbidden_websites__domains (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE moderation_condition__contains_links_outside_allowed_list__domains (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE moderation_condition__contains_links_outside_top100__allowed (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);

CREATE TABLE moderation_condition__floods_chat_or_exceeds_limits (
    condition_id             INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_characters           INTEGER NOT NULL DEFAULT 0,
    max_words                INTEGER NOT NULL DEFAULT 0,
    max_lines                INTEGER NOT NULL DEFAULT 0,
    chars_per_line           INTEGER NOT NULL DEFAULT 40,
    disallow_invisible_chars BOOLEAN NOT NULL DEFAULT 0,
    disallow_empty_messages  BOOLEAN NOT NULL DEFAULT 1
);

CREATE TABLE moderation_condition__user_exceeds_messages_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

CREATE TABLE moderation_condition__user_exceeds_moderation_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);

-- Flip the rule <-> action reference. An action row was already owned 1:1 by
-- its rule; only the foreign key pointed the other way, which is exactly why
-- orphaned action rows had to be collected and deleted by hand.
--
-- The column is nullable because that is the only kind of REFERENCES column
-- SQLite can add to a populated table (same reason `action_id` was nullable in
-- 0016). Rebuilding the table instead is not an option here: with foreign keys
-- enabled, DROP TABLE runs an implicit DELETE FROM, which would fire the
-- ON DELETE CASCADE on every `moderation_action__<name>` row before the
-- replacement could be renamed into place. The backfill below fills the column
-- for every surviving row and the writer always sets it.
ALTER TABLE moderation_actions
    ADD COLUMN rule_id INTEGER REFERENCES moderation_rules(id) ON DELETE CASCADE;

-- ---------------------------------------------------------------------------
-- Backfill
-- ---------------------------------------------------------------------------

-- One row per existing rule, with a deterministic new id. Every migrated rule
-- has exactly one condition, so the condition node reuses the same number: the
-- tables are separate, and it keeps every insert below a single join away.
CREATE TEMP TABLE _rule_map AS
SELECT src, old_id, group_id, rank, action_id, type,
       ROW_NUMBER() OVER (ORDER BY src, old_id) AS new_id
FROM (
    SELECT 'contains_banned_words' AS src, id AS old_id, group_id, rank, action_id,
           'ContainsBannedWords' AS type
      FROM moderation_rule__contains_banned_words
    UNION ALL
    SELECT 'contains_links_outside_allowed_list', id, group_id, rank, action_id,
           'ContainsLinksOutsideAllowedList'
      FROM moderation_rule__contains_links_outside_allowed_list
    UNION ALL
    SELECT 'contains_links_outside_top100', id, group_id, rank, action_id,
           'ContainsLinksOutsideTop100'
      FROM moderation_rule__contains_links_outside_top100
    UNION ALL
    SELECT 'contains_links_to_forbidden_websites', id, group_id, rank, action_id,
           'ContainsLinksToForbiddenWebsites'
      FROM moderation_rule__contains_links_to_forbidden_websites
    UNION ALL
    SELECT 'floods_chat_or_exceeds_limits', id, group_id, rank, action_id,
           'FloodsChatOrExceedsLimits'
      FROM moderation_rule__floods_chat_or_exceeds_limits
    UNION ALL
    SELECT 'matches_exact_message', id, group_id, rank, action_id,
           'MatchesExactMessage'
      FROM moderation_rule__matches_exact_message
    UNION ALL
    SELECT 'matches_regex', id, group_id, rank, action_id,
           'MatchesRegex'
      FROM moderation_rule__matches_regex
    UNION ALL
    SELECT 'user_exceeds_messages_rate_limit', id, group_id, rank, action_id,
           'UserExceedsMessagesRateLimit'
      FROM moderation_rule__user_exceeds_messages_rate_limit
    UNION ALL
    SELECT 'user_exceeds_moderation_rate_limit', id, group_id, rank, action_id,
           'UserExceedsModerationRateLimit'
      FROM moderation_rule__user_exceeds_moderation_rate_limit
);

INSERT INTO moderation_rules (id, group_id, rank)
    SELECT new_id, group_id, rank FROM _rule_map;

INSERT INTO moderation_conditions (id, rule_id, parent_id, rank, type)
    SELECT new_id, new_id, NULL, 0, type FROM _rule_map;

-- Settings and lists, per condition type.

INSERT INTO moderation_condition__contains_banned_words__keywords (condition_id, keyword)
    SELECT m.new_id, s.keyword
      FROM _rule_map m
      JOIN moderation_rule__contains_banned_words__keywords s ON s.rule_id = m.old_id
     WHERE m.src = 'contains_banned_words';

INSERT INTO moderation_condition__matches_exact_message (condition_id, case_sensitive)
    SELECT m.new_id, o.case_sensitive
      FROM _rule_map m
      JOIN moderation_rule__matches_exact_message o ON o.id = m.old_id
     WHERE m.src = 'matches_exact_message';
INSERT INTO moderation_condition__matches_exact_message__messages (condition_id, message)
    SELECT m.new_id, s.message
      FROM _rule_map m
      JOIN moderation_rule__matches_exact_message__messages s ON s.rule_id = m.old_id
     WHERE m.src = 'matches_exact_message';

INSERT INTO moderation_condition__matches_regex__patterns (condition_id, pattern)
    SELECT m.new_id, s.pattern
      FROM _rule_map m
      JOIN moderation_rule__matches_regex__patterns s ON s.rule_id = m.old_id
     WHERE m.src = 'matches_regex';

INSERT INTO moderation_condition__contains_links_to_forbidden_websites__domains (condition_id, domain)
    SELECT m.new_id, s.domain
      FROM _rule_map m
      JOIN moderation_rule__contains_links_to_forbidden_websites__domains s ON s.rule_id = m.old_id
     WHERE m.src = 'contains_links_to_forbidden_websites';

INSERT INTO moderation_condition__contains_links_outside_allowed_list__domains (condition_id, domain)
    SELECT m.new_id, s.domain
      FROM _rule_map m
      JOIN moderation_rule__contains_links_outside_allowed_list__domains s ON s.rule_id = m.old_id
     WHERE m.src = 'contains_links_outside_allowed_list';

INSERT INTO moderation_condition__contains_links_outside_top100__allowed (condition_id, domain)
    SELECT m.new_id, s.domain
      FROM _rule_map m
      JOIN moderation_rule__contains_links_outside_top100__allowed s ON s.rule_id = m.old_id
     WHERE m.src = 'contains_links_outside_top100';

INSERT INTO moderation_condition__floods_chat_or_exceeds_limits (
        condition_id, max_characters, max_words, max_lines, chars_per_line,
        disallow_invisible_chars, disallow_empty_messages)
    SELECT m.new_id,
           COALESCE(o.max_characters, 0),
           COALESCE(o.max_words, 0),
           COALESCE(o.max_lines, 0),
           COALESCE(o.chars_per_line, 40),
           COALESCE(o.disallow_invisible_chars, 0),
           COALESCE(o.disallow_empty_messages, 1)
      FROM _rule_map m
      JOIN moderation_rule__floods_chat_or_exceeds_limits o ON o.id = m.old_id
     WHERE m.src = 'floods_chat_or_exceeds_limits';

INSERT INTO moderation_condition__user_exceeds_messages_rate_limit (
        condition_id, message_count, time_window_minutes)
    SELECT m.new_id, o.message_count, o.time_window_minutes
      FROM _rule_map m
      JOIN moderation_rule__user_exceeds_messages_rate_limit o ON o.id = m.old_id
     WHERE m.src = 'user_exceeds_messages_rate_limit';

INSERT INTO moderation_condition__user_exceeds_moderation_rate_limit (
        condition_id, message_count, time_window_minutes)
    SELECT m.new_id, o.message_count, o.time_window_minutes
      FROM _rule_map m
      JOIN moderation_rule__user_exceeds_moderation_rate_limit o ON o.id = m.old_id
     WHERE m.src = 'user_exceeds_moderation_rate_limit';

-- Point each action at the rule that owned it.
UPDATE moderation_actions
   SET rule_id = (SELECT m.new_id FROM _rule_map m WHERE m.action_id = moderation_actions.id);

-- Anything still unlinked was already orphaned before this migration (the old
-- schema, with the reference pointing the other way, could leak action rows).
-- Removing them cascades into their settings tables and makes "every action
-- belongs to a rule" true from here on.
--
-- This is safe because the set of tables holding an `action_id` is exactly the
-- set created by the migrations in this directory: they are frozen once merged,
-- so any database is reproducible from them. A database that ran an *unreleased*
-- iteration of a migration can carry a leftover table that breaks this delete —
-- drop that table before starting the bot, rather than weakening the cleanup.

DELETE FROM moderation_actions WHERE rule_id IS NULL;

-- Restores the "at most one action per rule" guarantee that the old `action_id`
-- column gave by being a single column, and indexes the join used on read.
CREATE UNIQUE INDEX idx_moderation_actions_rule_id ON moderation_actions (rule_id);

DROP TABLE _rule_map;

-- ---------------------------------------------------------------------------
-- Remove the old per-condition rule tables, children before parents so foreign
-- key enforcement stays satisfied.
-- ---------------------------------------------------------------------------

DROP TABLE moderation_rule__contains_banned_words__keywords;
DROP TABLE moderation_rule__matches_exact_message__messages;
DROP TABLE moderation_rule__matches_regex__patterns;
DROP TABLE moderation_rule__contains_links_to_forbidden_websites__domains;
DROP TABLE moderation_rule__contains_links_outside_allowed_list__domains;
DROP TABLE moderation_rule__contains_links_outside_top100__allowed;

DROP TABLE moderation_rule__contains_banned_words;
DROP TABLE moderation_rule__matches_exact_message;
DROP TABLE moderation_rule__matches_regex;
DROP TABLE moderation_rule__contains_links_to_forbidden_websites;
DROP TABLE moderation_rule__contains_links_outside_allowed_list;
DROP TABLE moderation_rule__contains_links_outside_top100;
DROP TABLE moderation_rule__floods_chat_or_exceeds_limits;
DROP TABLE moderation_rule__user_exceeds_messages_rate_limit;
DROP TABLE moderation_rule__user_exceeds_moderation_rate_limit;
