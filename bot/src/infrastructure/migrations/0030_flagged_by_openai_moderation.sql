-- FlaggedByOpenAiModeration: the owner's OpenAI API key, stored as typed, and
-- one trigger per moderation category.
CREATE TABLE moderation_condition__flagged_by_openai_moderation (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    api_key      TEXT    NOT NULL
);

-- One row per category that is not off: NULL means "OpenAI decides", a number
-- is the owner's own minimum score in percent. A category with no row is off,
-- so a category OpenAI adds later reads as off for every stored rule.
CREATE TABLE moderation_condition__flagged_by_openai_moderation__categories (
    condition_id      INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    category          TEXT    NOT NULL,
    min_score_percent INTEGER NULL CHECK (min_score_percent BETWEEN 1 AND 100),
    PRIMARY KEY (condition_id, category)
);
