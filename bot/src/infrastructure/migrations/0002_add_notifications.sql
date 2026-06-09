-- Per-group toggle: whether the group owner receives a DM each time the bot
-- moderates (deletes) a message in that group. Defaults to off (0).
ALTER TABLE moderation_groups
    ADD COLUMN notifications_enabled INTEGER NOT NULL DEFAULT 0;
