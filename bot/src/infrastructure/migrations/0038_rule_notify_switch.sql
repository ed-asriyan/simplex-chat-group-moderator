-- Each rule gets its own Notifications switch, and the group's mode goes: what
-- the mode said for the whole group, each rule now says for itself with its
-- two switches, `enabled` (whether its actions are carried out) and `notify`
-- (whether the owner is told when it matches).
ALTER TABLE moderation_rules
    ADD COLUMN notify BOOLEAN NOT NULL DEFAULT 1
        CHECK (notify IN (0, 1));

-- A rule that was off was not checked at all: it stays fully off.
UPDATE moderation_rules
SET notify = 0
WHERE enabled = 0;

-- A group that acted without telling: its rules act without telling.
UPDATE moderation_rules
SET notify = 0
WHERE group_id IN (SELECT group_id FROM moderation_groups WHERE mode = 'silent');

-- A group that only told: its rules that were on only tell.
UPDATE moderation_rules
SET enabled = 0
WHERE group_id IN (SELECT group_id FROM moderation_groups WHERE mode = 'dry');

ALTER TABLE moderation_groups DROP COLUMN mode;
