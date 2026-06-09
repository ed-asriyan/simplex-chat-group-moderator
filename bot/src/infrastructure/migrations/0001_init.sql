-- Baseline schema. Uses IF NOT EXISTS so it is a safe no-op on databases that
-- were already created by the legacy `SqliteModerationRepository::init()`.
CREATE TABLE IF NOT EXISTS moderation_groups (
    group_id           INTEGER PRIMARY KEY,
    messenger_group_id INTEGER NOT NULL UNIQUE,
    owner_id           INTEGER NOT NULL,
    group_name         TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_moderation_groups_owner_id
    ON moderation_groups (owner_id);

CREATE TABLE IF NOT EXISTS moderation_group_keywords (
    group_id INTEGER NOT NULL,
    keyword  TEXT NOT NULL,
    PRIMARY KEY (group_id, keyword)
);
