-- How hard the two OpenAI conditions try before reading a failure as "no
-- verdict": the number of calls in all and the pause between them, in seconds.
-- Rules stored before this keep the old behaviour of three tries a second
-- apart.
ALTER TABLE moderation_condition__flagged_by_omni_moderation
    ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 5);
ALTER TABLE moderation_condition__flagged_by_omni_moderation
    ADD COLUMN retry_delay_seconds INTEGER NOT NULL DEFAULT 1 CHECK (retry_delay_seconds BETWEEN 0 AND 10);

ALTER TABLE moderation_condition__flagged_by_openai_instruction
    ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 5);
ALTER TABLE moderation_condition__flagged_by_openai_instruction
    ADD COLUMN retry_delay_seconds INTEGER NOT NULL DEFAULT 1 CHECK (retry_delay_seconds BETWEEN 0 AND 10);
