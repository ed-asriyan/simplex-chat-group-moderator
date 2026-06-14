-- Per-group toggle: when dry mode is on, the bot performs all moderation checks
-- and sends notifications but does NOT actually delete messages. Defaults to off (0).
ALTER TABLE moderation_groups
    ADD COLUMN dry_mode_enabled INTEGER NOT NULL DEFAULT 0;
