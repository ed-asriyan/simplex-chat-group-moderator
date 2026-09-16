CREATE TABLE moderation_condition__author_hits_character_rate_limit (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    character_count     INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL
);
