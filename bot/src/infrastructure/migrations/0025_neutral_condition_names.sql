-- Name every condition after what it detects, not after what the owner thinks
-- of it.
--
-- Since 0022 conditions compose with All / Any / Not, and a judgement baked
-- into a name reads backwards under a Not ("not: contains banned words"). The
-- renamed conditions keep their rows; only the `type` tag and the table names
-- change.
--
-- One condition is not just renamed: FloodsChatOrExceedsLimits bundled five
-- independent checks behind a single "any of these", which under a Not became
-- "none of these" without saying so. It is split into one condition per check,
-- and each old node becomes an `Any` of the checks it had enabled.

-- ---------------------------------------------------------------------------
-- Renames
-- ---------------------------------------------------------------------------

UPDATE moderation_conditions SET type = 'ContainsWords' WHERE type = 'ContainsBannedWords';
ALTER TABLE moderation_condition__contains_banned_words__keywords
    RENAME TO moderation_condition__contains_words__keywords;

UPDATE moderation_conditions SET type = 'ContainsLinksInList'
 WHERE type = 'ContainsLinksToForbiddenWebsites';
ALTER TABLE moderation_condition__contains_links_to_forbidden_websites__domains
    RENAME TO moderation_condition__contains_links_in_list__domains;

UPDATE moderation_conditions SET type = 'ContainsLinksOutsideList'
 WHERE type = 'ContainsLinksOutsideAllowedList';
ALTER TABLE moderation_condition__contains_links_outside_allowed_list__domains
    RENAME TO moderation_condition__contains_links_outside_list__domains;

-- The type keeps its name. Its list subtable is named after the field, which is
-- now `domains` (formerly `allowed`) like on the other two link conditions.
ALTER TABLE moderation_condition__contains_links_outside_top100__allowed
    RENAME TO moderation_condition__contains_links_outside_top100__domains;

UPDATE moderation_conditions SET type = 'AuthorHitsMessageRateLimit'
 WHERE type = 'UserExceedsMessagesRateLimit';
ALTER TABLE moderation_condition__user_exceeds_messages_rate_limit
    RENAME TO moderation_condition__author_hits_message_rate_limit;

UPDATE moderation_conditions SET type = 'AuthorHitsModerationRateLimit'
 WHERE type = 'UserExceedsModerationRateLimit';
ALTER TABLE moderation_condition__user_exceeds_moderation_rate_limit
    RENAME TO moderation_condition__author_hits_moderation_rate_limit;

UPDATE moderation_conditions SET type = 'AuthorJoinedRecently' WHERE type = 'UserJoinedRecently';
ALTER TABLE moderation_condition__user_joined_recently
    RENAME TO moderation_condition__author_joined_recently;

-- ---------------------------------------------------------------------------
-- Split FloodsChatOrExceedsLimits
-- ---------------------------------------------------------------------------

-- IsBlank and ContainsInvisibleCharacters take no parameters, so like the
-- composites they get no table: the registry row is all there is to them.
CREATE TABLE moderation_condition__exceeds_max_characters (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_characters INTEGER NOT NULL
);

CREATE TABLE moderation_condition__exceeds_max_words (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_words    INTEGER NOT NULL
);

CREATE TABLE moderation_condition__exceeds_max_lines (
    condition_id   INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    max_lines      INTEGER NOT NULL,
    chars_per_line INTEGER NOT NULL
);

-- Every old node with its settings. Driven by the registry rather than the
-- settings table, so a node that somehow lost its settings row is still
-- converted, with the defaults the reader used to fall back to.
CREATE TEMP TABLE _floods AS
SELECT c.id AS node_id,
       COALESCE(f.max_characters, 0)           AS max_characters,
       COALESCE(f.max_words, 0)                AS max_words,
       COALESCE(f.max_lines, 0)                AS max_lines,
       COALESCE(f.chars_per_line, 40)          AS chars_per_line,
       COALESCE(f.disallow_invisible_chars, 0) AS disallow_invisible_chars,
       COALESCE(f.disallow_empty_messages, 1)  AS disallow_empty_messages
  FROM moderation_conditions c
  LEFT JOIN moderation_condition__floods_chat_or_exceeds_limits f ON f.condition_id = c.id
 WHERE c.type = 'FloodsChatOrExceedsLimits';

-- One row per enabled check (a zero maximum meant "off"), ranked in the order
-- the old condition ran them, so a message reports the same reason as before.
CREATE TEMP TABLE _flood_parts AS
SELECT node_id, type, ROW_NUMBER() OVER (PARTITION BY node_id ORDER BY step) - 1 AS rank
FROM (
    SELECT node_id, 0 AS step, 'IsBlank' AS type FROM _floods WHERE disallow_empty_messages
    UNION ALL
    SELECT node_id, 1, 'ContainsInvisibleCharacters' FROM _floods WHERE disallow_invisible_chars
    UNION ALL
    SELECT node_id, 2, 'ExceedsMaxCharacters' FROM _floods WHERE max_characters > 0
    UNION ALL
    SELECT node_id, 3, 'ExceedsMaxWords' FROM _floods WHERE max_words > 0
    UNION ALL
    SELECT node_id, 4, 'ExceedsMaxLines' FROM _floods WHERE max_lines > 0
);

CREATE TEMP TABLE _flood_part_counts AS
SELECT f.node_id,
       (SELECT COUNT(*) FROM _flood_parts p WHERE p.node_id = f.node_id) AS parts
  FROM _floods f;

-- A single enabled check replaces the node in place: the tree stays canonical
-- (no single-child composites) and the node keeps its parent and rank.
UPDATE moderation_conditions
   SET type = (SELECT p.type FROM _flood_parts p WHERE p.node_id = moderation_conditions.id)
 WHERE id IN (SELECT node_id FROM _flood_part_counts WHERE parts = 1);

-- Otherwise the node becomes an `Any` over its checks. With no check enabled it
-- stays an empty `Any`, which never matches, exactly like the old condition
-- did; the next save by the owner drops it as an empty composite.
UPDATE moderation_conditions SET type = 'Any'
 WHERE id IN (SELECT node_id FROM _flood_part_counts WHERE parts <> 1);

INSERT INTO moderation_conditions (rule_id, parent_id, rank, type)
SELECT c.rule_id, p.node_id, p.rank, p.type
  FROM _flood_parts p
  JOIN _flood_part_counts n ON n.node_id = p.node_id
  JOIN moderation_conditions c ON c.id = p.node_id
 WHERE n.parts > 1;

-- The node carrying a check is either the old node itself (single check) or
-- one of its new children. Nothing else can have one of these brand-new types,
-- and the old node was a leaf, so it has no other children to confuse this.
INSERT INTO moderation_condition__exceeds_max_characters (condition_id, max_characters)
SELECT c.id, f.max_characters
  FROM moderation_conditions c
  JOIN _floods f ON f.node_id = c.id OR f.node_id = c.parent_id
 WHERE c.type = 'ExceedsMaxCharacters';

INSERT INTO moderation_condition__exceeds_max_words (condition_id, max_words)
SELECT c.id, f.max_words
  FROM moderation_conditions c
  JOIN _floods f ON f.node_id = c.id OR f.node_id = c.parent_id
 WHERE c.type = 'ExceedsMaxWords';

INSERT INTO moderation_condition__exceeds_max_lines (condition_id, max_lines, chars_per_line)
SELECT c.id, f.max_lines, f.chars_per_line
  FROM moderation_conditions c
  JOIN _floods f ON f.node_id = c.id OR f.node_id = c.parent_id
 WHERE c.type = 'ExceedsMaxLines';

DROP TABLE _flood_part_counts;
DROP TABLE _flood_parts;
DROP TABLE _floods;

DROP TABLE moderation_condition__floods_chat_or_exceeds_limits;
