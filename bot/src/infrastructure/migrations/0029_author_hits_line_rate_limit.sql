CREATE TABLE moderation_condition__author_hits_line_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    line_count          INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL,
    chars_per_line      INTEGER NOT NULL
);
