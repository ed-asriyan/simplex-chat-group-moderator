-- The two per-group toggles become one mode: 'dry' (check and tell the owner,
-- but do nothing), 'silent' (act without telling the owner) or 'notifications'
-- (act and tell the owner). Dry mode used to force notifications on, so a dry
-- group stays dry whatever its notification toggle says.
ALTER TABLE moderation_groups
    ADD COLUMN mode TEXT NOT NULL DEFAULT 'notifications'
        CHECK (mode IN ('dry', 'silent', 'notifications'));

UPDATE moderation_groups
SET mode = CASE
    WHEN dry_mode_enabled != 0 THEN 'dry'
    WHEN notifications_enabled = 0 THEN 'silent'
    ELSE 'notifications'
END;

ALTER TABLE moderation_groups DROP COLUMN notifications_enabled;
ALTER TABLE moderation_groups DROP COLUMN dry_mode_enabled;
