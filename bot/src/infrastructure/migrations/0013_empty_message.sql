CREATE TABLE moderation_rule__empty_message (
    id       INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank     INTEGER NOT NULL
);
CREATE INDEX idx_moderation_rule__empty_message_group_id
    ON moderation_rule__empty_message (group_id);
