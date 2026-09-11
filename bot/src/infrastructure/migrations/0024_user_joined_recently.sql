CREATE TABLE moderation_condition__user_joined_recently (
    condition_id        INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    time_window_minutes INTEGER NOT NULL
);
