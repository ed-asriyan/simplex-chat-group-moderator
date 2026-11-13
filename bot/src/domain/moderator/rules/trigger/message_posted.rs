//! `Trigger::MessagePosted`: a member's message, new or edited, checked as it
//! is (US-D1). Its rules read the message itself, and the member who wrote it.

use crate::domain::moderator::ports::GroupMessage;
use crate::domain::moderator::rules::moderation_action::trigger_actions;
use crate::domain::moderator::rules::moderation_condition::trigger_conditions;
use crate::domain::moderator::rules::trigger::Trigger;

trigger_conditions! {
    MessageCondition on GroupMessage, Trigger::MessagePosted;
    ContainsWords,
    MatchesExactMessage,
    MatchesRegex,
    ContainsRepeatedSequence,
    ContainsLinksInList,
    ContainsLinksOutsideList,
    ContainsLinksOutsideTop100,
    IsBlank,
    ContainsInvisibleCharacters,
    ContainsImage,
    ContainsVideo,
    ContainsVoiceMessage,
    ContainsFile,
    ExceedsMaxCharacters,
    ExceedsMaxWords,
    ExceedsMaxLines,
    AuthorHitsMessageRateLimit,
    AuthorHitsCharacterRateLimit,
    AuthorHitsLineRateLimit,
    AuthorHitsModerationRateLimit,
    GroupHitsMessageRateLimit,
    GroupHitsCharacterRateLimit,
    GroupHitsLineRateLimit,
    AuthorJoinedRecently,
    MemberNameContainsWords,
    MemberNameContainsLinksOutsideList,
    FlaggedByOmniModeration,
    FlaggedByOpenRouterInstruction,
}

trigger_actions! {
    MessageAction on GroupMessage, Trigger::MessagePosted;
    ModerateMessage,
    SetAuthorObserver,
    KickAuthor,
}
