-- Rename rule condition tables to logical predicates.
--
-- The previous names used action-style blacklist/whitelist terminology.
-- Rules are now typed pairs of (Action, Condition), where the condition
-- represents a pure logical predicate evaluated against incoming messages.

-- ContainsBannedWords
ALTER TABLE moderation_rule__words_blacklist
    RENAME TO moderation_rule__contains_banned_words;
ALTER TABLE moderation_rule__words_blacklist__keywords
    RENAME TO moderation_rule__contains_banned_words__keywords;
DROP INDEX IF EXISTS idx_moderation_rule__words_blacklist_group_id;
CREATE INDEX idx_moderation_rule__contains_banned_words_group_id
    ON moderation_rule__contains_banned_words (group_id);

-- MatchesExactMessage
ALTER TABLE moderation_rule__messages_blacklist
    RENAME TO moderation_rule__matches_exact_message;
ALTER TABLE moderation_rule__messages_blacklist__messages
    RENAME TO moderation_rule__matches_exact_message__messages;
DROP INDEX IF EXISTS idx_moderation_rule__messages_blacklist_group_id;
CREATE INDEX idx_moderation_rule__matches_exact_message_group_id
    ON moderation_rule__matches_exact_message (group_id);

-- ContainsLinksToForbiddenWebsites
ALTER TABLE moderation_rule__links_blacklist
    RENAME TO moderation_rule__contains_links_to_forbidden_websites;
ALTER TABLE moderation_rule__links_blacklist__domains
    RENAME TO moderation_rule__contains_links_to_forbidden_websites__domains;
DROP INDEX IF EXISTS idx_moderation_rule__links_blacklist_group_id;
CREATE INDEX idx_moderation_rule__contains_links_to_forbidden_websites_group_id
    ON moderation_rule__contains_links_to_forbidden_websites (group_id);

-- ContainsLinksOutsideAllowedList
ALTER TABLE moderation_rule__links_whitelist
    RENAME TO moderation_rule__contains_links_outside_allowed_list;
ALTER TABLE moderation_rule__links_whitelist__domains
    RENAME TO moderation_rule__contains_links_outside_allowed_list__domains;
DROP INDEX IF EXISTS idx_moderation_rule__links_whitelist_group_id;
CREATE INDEX idx_moderation_rule__contains_links_outside_allowed_list_group_id
    ON moderation_rule__contains_links_outside_allowed_list (group_id);

-- ContainsLinksOutsideTop100
ALTER TABLE moderation_rule__links_whitelist_top100
    RENAME TO moderation_rule__contains_links_outside_top100;
ALTER TABLE moderation_rule__links_whitelist_top100__allowed
    RENAME TO moderation_rule__contains_links_outside_top100__allowed;
DROP INDEX IF EXISTS idx_moderation_rule__links_whitelist_top100_group_id;
CREATE INDEX idx_moderation_rule__contains_links_outside_top100_group_id
    ON moderation_rule__contains_links_outside_top100 (group_id);

-- FloodsChatOrExceedsLimits
ALTER TABLE moderation_rule__screen_flooding
    RENAME TO moderation_rule__floods_chat_or_exceeds_limits;
DROP INDEX IF EXISTS idx_moderation_rule__screen_flooding_group_id;
CREATE INDEX idx_moderation_rule__floods_chat_or_exceeds_limits_group_id
    ON moderation_rule__floods_chat_or_exceeds_limits (group_id);
