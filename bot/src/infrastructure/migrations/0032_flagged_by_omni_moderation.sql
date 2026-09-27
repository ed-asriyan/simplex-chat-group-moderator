-- FlaggedByOpenAiModeration is now FlaggedByOmniModeration: named after the
-- omni moderation model it asks, now that a second OpenAI condition exists.
-- Stored rows are renamed in place; no link carries the old tag, because the
-- bot always sends a freshly generated one.
UPDATE moderation_conditions
   SET type = 'FlaggedByOmniModeration'
 WHERE type = 'FlaggedByOpenAiModeration';

ALTER TABLE moderation_condition__flagged_by_openai_moderation
    RENAME TO moderation_condition__flagged_by_omni_moderation;
ALTER TABLE moderation_condition__flagged_by_openai_moderation__categories
    RENAME TO moderation_condition__flagged_by_omni_moderation__categories;
