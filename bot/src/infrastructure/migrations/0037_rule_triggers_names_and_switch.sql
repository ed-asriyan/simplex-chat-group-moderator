-- A rule gets what makes the bot check it, a name of the owner's, and an on/off
-- switch. Every rule stored so far was checked on each message and was on, so
-- the defaults are what they already were; none has a name.
ALTER TABLE moderation_rules
    ADD COLUMN name TEXT
        CHECK (name IS NULL OR length(name) BETWEEN 1 AND 60);

ALTER TABLE moderation_rules
    ADD COLUMN trigger TEXT NOT NULL DEFAULT 'message_posted'
        CHECK (trigger IN ('message_posted', 'member_joined'));

ALTER TABLE moderation_rules
    ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT 1
        CHECK (enabled IN (0, 1));

-- A rule for joins may have no condition: it then has no row in
-- moderation_conditions, which needs no change.

-- The two conditions on a member's name, each a list only.
CREATE TABLE moderation_condition__member_name_contains_words__keywords (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    keyword      TEXT    NOT NULL,
    PRIMARY KEY (condition_id, keyword)
);

CREATE TABLE moderation_condition__member_name_contains_links_outside_list__domains (
    condition_id INTEGER NOT NULL REFERENCES moderation_conditions(id) ON DELETE CASCADE,
    domain       TEXT    NOT NULL,
    PRIMARY KEY (condition_id, domain)
);
