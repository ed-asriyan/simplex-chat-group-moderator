CREATE TABLE moderation_rule__user_exceeds_messages_rate_limit (
    id                  INTEGER PRIMARY KEY,
    group_id            INTEGER NOT NULL REFERENCES moderation_groups(group_id) ON DELETE CASCADE,
    rank                INTEGER NOT NULL,
    message_count       INTEGER NOT NULL,
    time_window_minutes INTEGER NOT NULL,
    action_id           INTEGER REFERENCES moderation_actions(id)
);
CREATE INDEX idx_moderation_rule__user_exceeds_messages_rate_limit_group_id
    ON moderation_rule__user_exceeds_messages_rate_limit (group_id);
