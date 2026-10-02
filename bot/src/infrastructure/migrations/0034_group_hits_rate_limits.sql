CREATE TABLE moderation_condition__group_hits_message_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);
CREATE TABLE moderation_condition__group_hits_character_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    character_count     INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);
CREATE TABLE moderation_condition__group_hits_line_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    line_count          INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL,
    chars_per_line      INTEGER NOT NULL
);
