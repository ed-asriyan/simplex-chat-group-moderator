-- Add an ON DELETE CASCADE foreign key from keywords to their group so that
-- removing a group automatically removes its keywords. SQLite cannot add a
-- foreign key with ALTER TABLE, so the table is rebuilt in place.
CREATE TABLE moderation_group_keywords_new (
    group_id INTEGER NOT NULL,
    keyword  TEXT NOT NULL,
    PRIMARY KEY (group_id, keyword),
    FOREIGN KEY (group_id) REFERENCES moderation_groups (group_id) ON DELETE CASCADE
);

-- Copy over existing keywords, dropping any orphans that reference a group that
-- no longer exists (these would violate the new foreign key).
INSERT INTO moderation_group_keywords_new (group_id, keyword)
    SELECT k.group_id, k.keyword
    FROM moderation_group_keywords k
    WHERE k.group_id IN (SELECT group_id FROM moderation_groups);

DROP TABLE moderation_group_keywords;

ALTER TABLE moderation_group_keywords_new RENAME TO moderation_group_keywords;
