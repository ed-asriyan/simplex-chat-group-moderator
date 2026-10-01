-- FlaggedByOpenAiInstruction is now FlaggedByOpenRouterInstruction: the model
-- following the owner's instruction is asked through OpenRouter, while
-- FlaggedByOmniModeration stays on OpenAI. Stored rows are renamed in place; no
-- link carries the old tag, because the bot always sends a freshly generated
-- one.
UPDATE moderation_conditions
   SET type = 'FlaggedByOpenRouterInstruction'
 WHERE type = 'FlaggedByOpenAiInstruction';

ALTER TABLE moderation_condition__flagged_by_openai_instruction
    RENAME TO moderation_condition__flagged_by_openrouter_instruction;

-- OpenRouter names a model after its vendor. Every OpenAI model the old
-- condition could use is on OpenRouter under the same name, so the stored
-- choice is kept. The stored key is an OpenAI key, which OpenRouter does not
-- take: the owner is told so the next time the rules are saved.
UPDATE moderation_condition__flagged_by_openrouter_instruction
   SET model = 'openai/' || model
 WHERE model NOT LIKE '%/%';
