CREATE TABLE moderation_rule__screen_flooding (
    id                       INTEGER PRIMARY KEY,
    group_id                 INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank                     INTEGER NOT NULL,
    max_characters           INTEGER,
    max_words                INTEGER,
    max_lines                INTEGER,
    chars_per_line           INTEGER,
    disallow_invisible_chars BOOLEAN NOT NULL DEFAULT 0,
    disallow_empty_messages  BOOLEAN NOT NULL DEFAULT 1
);
CREATE INDEX idx_moderation_rule__screen_flooding_group_id
    ON moderation_rule__screen_flooding (group_id);

INSERT INTO moderation_rule__screen_flooding (id, group_id, rank, disallow_empty_messages)
    SELECT id, group_id, rank, 1 FROM moderation_rule__empty_message;

DROP TABLE moderation_rule__empty_message;
