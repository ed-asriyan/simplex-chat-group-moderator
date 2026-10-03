-- How many of the group's earlier messages FlaggedByOpenRouterInstruction sends
-- along with the one it judges, for the model to read as context. Rules stored
-- before this send none, as they did.
ALTER TABLE moderation_condition__flagged_by_openrouter_instruction
    ADD COLUMN context_messages INTEGER NOT NULL DEFAULT 0 CHECK (context_messages >= 0);
