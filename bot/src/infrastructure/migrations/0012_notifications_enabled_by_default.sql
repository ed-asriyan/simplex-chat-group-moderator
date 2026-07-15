ALTER TABLE moderation_groups 
ADD COLUMN notifications_enabled_new INTEGER DEFAULT 1;

UPDATE moderation_groups 
SET notifications_enabled_new = notifications_enabled;

ALTER TABLE moderation_groups 
DROP COLUMN notifications_enabled;

ALTER TABLE moderation_groups 
RENAME COLUMN notifications_enabled_new TO notifications_enabled;
