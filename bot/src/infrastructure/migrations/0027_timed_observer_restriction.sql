-- Let `SetAuthorObserver` hold the author for a limited time.
--
-- Before: the action had no settings — an author made an observer stayed one
-- until the owner changed their role by hand.
-- After: it carries `duration_minutes`, where 0 keeps the old "indefinitely"
-- behaviour and N schedules the bot to restore the author to member N minutes
-- later. Existing rows get 0, so every group keeps exactly what it has today.

-- The default is what carries the existing rows across, and is required anyway:
-- SQLite only accepts a NOT NULL column added to a populated table when it has
-- one. 0026 had to rebuild this table because it *changed* a column; adding one
-- needs no rebuild.
ALTER TABLE moderation_action__set_author_observer
    ADD COLUMN duration_minutes INTEGER NOT NULL DEFAULT 0;

-- The restores the bot still owes: one row per member currently held as an
-- observer with a deadline, removed once the bot has restored them.
--
-- This is not configuration, so it hangs off the group rather than off the
-- action that scheduled it: saving rules deletes and reinserts every
-- `moderation_rules` row of a group, and a restore anchored to an action row
-- would be cascaded away with it — leaving that member an observer forever.
-- Anchoring it to `moderation_groups` keeps the obligation across rule edits,
-- while a deleted group still takes its pending restores with it.
CREATE TABLE moderation_set_author_observer_restores (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id   INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    member_id  INTEGER NOT NULL,
    -- Unix seconds, UTC. Nobody reads this column by eye, and an integer
    -- compares and sorts without a text format to agree on first
    -- (`datetime(execute_at, 'unixepoch')` renders it when someone does look).
    execute_at INTEGER NOT NULL,
    -- One deadline per member: a later restriction of the same member replaces
    -- the pending restore instead of adding a second one.
    UNIQUE (group_id, member_id)
);

CREATE INDEX idx_moderation_set_author_observer_restores_execute_at
    ON moderation_set_author_observer_restores (execute_at);
