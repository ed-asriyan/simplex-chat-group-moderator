-- Turn a rule's single action into an ordered list of actions, and flatten the
-- actions themselves.
--
-- Before: a rule had at most one action (enforced by a unique index on
-- `moderation_actions.rule_id`), and two of the three action types carried a
-- nested "...and what about the message?" setting:
--   KickAuthor.delete_messages    0 = None, 1 = TriggeredMessage, 2 = AllMessages
--   SetAuthorObserver.delete_message  0 = None, 1 = TriggeredMessage
-- Moderating the triggering message was therefore expressible twice: as the
-- `ModerateMessage` action, and as a sub-option of two other actions.
--
-- After: a rule owns several `moderation_actions` rows ordered by `rank`, each
-- action is one indivisible thing, and "also moderate the message" is simply the
-- `ModerateMessage` action sitting next to the other one:
--   KickAuthor.delete_all_messages  0/1 — the only choice a kick still carries
--   SetAuthorObserver               no settings at all
--
-- The stored rules are rewritten so every group keeps exactly the behaviour it
-- has today:
--   ModerateMessage                      -> [ModerateMessage]
--   KickAuthor        / delete_messages=0 -> [KickAuthor(all=0)]
--   KickAuthor        / delete_messages=1 -> [ModerateMessage, KickAuthor(all=0)]
--   KickAuthor        / delete_messages=2 -> [KickAuthor(all=1)]
--   SetAuthorObserver / delete_message=0  -> [SetAuthorObserver]
--   SetAuthorObserver / delete_message=1  -> [SetAuthorObserver, ModerateMessage]
--
-- `rank` follows the order the actions are executed in (observer, then moderate,
-- then kick — kicking last, because once the author is gone acting on them or on
-- their message may no longer be possible), which is why the two split cases put
-- the new `ModerateMessage` row on different sides of the action it joins.

-- ---------------------------------------------------------------------------
-- Which actions have to be split, captured before the settings tables that
-- carry the answer are rebuilt below.
-- ---------------------------------------------------------------------------

-- The predicates mirror how the reader being replaced interpreted these
-- columns, rather than the values the writer produced: it mapped kick's 0 to
-- None and 2 to AllMessages and *everything else* to TriggeredMessage, and
-- observer's 0 to None and everything else to TriggeredMessage. Only 0, 1 and 2
-- were ever written, so the difference is theoretical — but a migration is the
-- one thing that cannot be corrected afterwards, so it matches the old meaning
-- for every value a row could hold.
CREATE TEMP TABLE _split_actions AS
SELECT a.id AS action_id, a.rule_id AS rule_id, 1 AS existing_rank, 0 AS moderate_rank
  FROM moderation_actions a
  JOIN moderation_action__kick_author k ON k.action_id = a.id
 WHERE a.type = 'KickAuthor' AND k.delete_messages NOT IN (0, 2)
UNION ALL
SELECT a.id, a.rule_id, 0, 1
  FROM moderation_actions a
  JOIN moderation_action__set_author_observer o ON o.action_id = a.id
 WHERE a.type = 'SetAuthorObserver' AND o.delete_message <> 0;

-- ---------------------------------------------------------------------------
-- Structure: several actions per rule, ordered
-- ---------------------------------------------------------------------------

-- A default is required to add a NOT NULL column to a populated table; every
-- action that is not split keeps rank 0, which is exactly what the default says.
ALTER TABLE moderation_actions ADD COLUMN rank INTEGER NOT NULL DEFAULT 0;

-- The old index existed to enforce "at most one action per rule". Replacing it
-- with (rule_id, rank) lifts that limit while keeping the rule_id lookup indexed
-- (rule_id is the prefix) and keeping ranks unique within a rule, mirroring
-- `moderation_rules.rank` within a group.
DROP INDEX idx_moderation_actions_rule_id;
CREATE UNIQUE INDEX idx_moderation_actions_rule_id_rank ON moderation_actions (rule_id, rank);

-- ---------------------------------------------------------------------------
-- Per-action settings tables
-- ---------------------------------------------------------------------------

-- Both tables are leaves of the schema tree: nothing references them, so the
-- implicit DELETE FROM that DROP TABLE performs with foreign keys enabled has no
-- cascade to fire (unlike `moderation_actions` itself, which is why 0022 could
-- not be rebuilt this way).

-- KickAuthor: the tri-state collapses to "delete every message of theirs, or not".
-- Cases 0 and 1 both become 0 — case 1's message deletion is carried by the
-- `ModerateMessage` action added below.
CREATE TABLE moderation_action__kick_author_v2 (
    action_id           INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE,
    delete_all_messages BOOLEAN NOT NULL DEFAULT 0
);
INSERT INTO moderation_action__kick_author_v2 (action_id, delete_all_messages)
    SELECT action_id, CASE WHEN delete_messages = 2 THEN 1 ELSE 0 END
      FROM moderation_action__kick_author;
DROP TABLE moderation_action__kick_author;
ALTER TABLE moderation_action__kick_author_v2 RENAME TO moderation_action__kick_author;

-- SetAuthorObserver: no settings left. The table stays (a zero-setting action
-- still gets one, like `moderation_action__moderate_message`) with only its key.
CREATE TABLE moderation_action__set_author_observer_v2 (
    action_id INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE
);
INSERT INTO moderation_action__set_author_observer_v2 (action_id)
    SELECT action_id FROM moderation_action__set_author_observer;
DROP TABLE moderation_action__set_author_observer;
ALTER TABLE moderation_action__set_author_observer_v2
    RENAME TO moderation_action__set_author_observer;

-- ---------------------------------------------------------------------------
-- Backfill: give each split action its `ModerateMessage` sibling
-- ---------------------------------------------------------------------------

-- Move the existing action to its place in the execution order first, so the
-- new rank-taking row below never collides with it.
UPDATE moderation_actions
   SET rank = (SELECT s.existing_rank FROM _split_actions s WHERE s.action_id = moderation_actions.id)
 WHERE id IN (SELECT action_id FROM _split_actions);

-- Explicit ids (max + ROW_NUMBER) keep the inserts below deterministic and let
-- the registry row and its settings row be written from the same mapping,
-- instead of relying on insert-order rowid assignment.
CREATE TEMP TABLE _new_moderate_actions AS
SELECT s.rule_id AS rule_id,
       s.moderate_rank AS rank,
       (SELECT COALESCE(MAX(id), 0) FROM moderation_actions)
           + ROW_NUMBER() OVER (ORDER BY s.action_id) AS new_id
  FROM _split_actions s;

INSERT INTO moderation_actions (id, rule_id, rank, type)
    SELECT new_id, rule_id, rank, 'ModerateMessage' FROM _new_moderate_actions;
INSERT INTO moderation_action__moderate_message (action_id)
    SELECT new_id FROM _new_moderate_actions;

DROP TABLE _new_moderate_actions;
DROP TABLE _split_actions;
