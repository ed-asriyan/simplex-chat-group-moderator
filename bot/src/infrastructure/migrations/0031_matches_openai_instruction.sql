-- MatchesOpenAiInstruction: the owner's OpenAI API key, stored as typed, the
-- model to ask, and the instruction it is given for every message.
CREATE TABLE moderation_condition__matches_openai_instruction (
    condition_id INTEGER PRIMARY KEY REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    api_key      TEXT    NOT NULL,
    model        TEXT    NOT NULL,
    instruction  TEXT    NOT NULL
);
