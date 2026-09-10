CREATE TABLE moderation_rule__matches_regex (
    id        INTEGER PRIMARY KEY,
    group_id  INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank      INTEGER NOT NULL,
    action_id INTEGER REFERENCES moderation_actions(id)
);
CREATE INDEX idx_moderation_rule__matches_regex_group_id
    ON moderation_rule__matches_regex (group_id);

CREATE TABLE moderation_rule__matches_regex__patterns (
    rule_id INTEGER NOT NULL REFERENCES moderation_rule__matches_regex(id) ON DELETE CASCADE,
    pattern TEXT NOT NULL,
    PRIMARY KEY (rule_id, pattern)
);
