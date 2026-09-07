CREATE TABLE moderation_rule__screen_flooding_new (
    id                       INTEGER PRIMARY KEY,
    group_id                 INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank                     INTEGER NOT NULL,
    max_characters           INTEGER NOT NULL DEFAULT 0,
    max_words                INTEGER NOT NULL DEFAULT 0,
    max_lines                INTEGER NOT NULL DEFAULT 0,
    chars_per_line           INTEGER NOT NULL DEFAULT 40,
    disallow_invisible_chars BOOLEAN NOT NULL DEFAULT 0,
    disallow_empty_messages  BOOLEAN NOT NULL DEFAULT 1
);

INSERT INTO moderation_rule__screen_flooding_new (
    id, group_id, rank, max_characters, max_words, max_lines, chars_per_line, disallow_invisible_chars, disallow_empty_messages
)
SELECT
    id,
    group_id,
    rank,
    COALESCE(max_characters, 0),
    COALESCE(max_words, 0),
    COALESCE(max_lines, 0),
    COALESCE(chars_per_line, 40),
    COALESCE(disallow_invisible_chars, 0),
    COALESCE(disallow_empty_messages, 1)
FROM moderation_rule__screen_flooding;

DROP TABLE moderation_rule__screen_flooding;

ALTER TABLE moderation_rule__screen_flooding_new RENAME TO moderation_rule__screen_flooding;

CREATE INDEX idx_moderation_rule__screen_flooding_group_id
    ON moderation_rule__screen_flooding (group_id);
