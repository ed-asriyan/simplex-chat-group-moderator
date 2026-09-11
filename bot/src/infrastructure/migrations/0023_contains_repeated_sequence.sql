CREATE TABLE moderation_condition__contains_repeated_sequence (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    min_repeats  INTEGER NOT NULL,
    min_length   INTEGER NOT NULL
);
