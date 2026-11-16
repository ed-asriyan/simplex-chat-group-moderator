-- ✉️ Send the member a message (US-F5): the owner's text, sent to the
-- message's author in the group's chat with admins. Not empty, at most 1000
-- characters (US-C2).
CREATE TABLE moderation_action__send_member_message (
    action_id INTEGER PRIMARY KEY
        REFERENCES moderation_actions(id) ON DELETE CASCADE,
    text      TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 1000)
);
