-- SetAuthorObserver: specifies what to do with the author's message:
-- 0: None (do not delete)
-- 1: TriggeredMessage (delete offending message)
CREATE TABLE moderation_action__set_author_observer (
    action_id      INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE,
    delete_message INTEGER NOT NULL
);
