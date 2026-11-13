//! `Trigger::MemberJoined`: someone who joined as a member or an observer
//! (US-D6). There is no message yet, so its rules read only the member — their
//! name (US-E31, US-E32) — and act only on the member (US-F1–US-F3). A rule
//! for it may have no condition, and then matches everyone who joins.

use crate::domain::moderator::ports::MemberJoin;
use crate::domain::moderator::rules::moderation_action::trigger_actions;
use crate::domain::moderator::rules::moderation_condition::trigger_conditions;
use crate::domain::moderator::rules::trigger::Trigger;

trigger_conditions! {
    JoinCondition on MemberJoin, Trigger::MemberJoined;
    MemberNameContainsWords,
    MemberNameContainsLinksOutsideList,
}

trigger_actions! {
    JoinAction on MemberJoin, Trigger::MemberJoined;
    SetAuthorObserver,
    KickAuthor,
}
