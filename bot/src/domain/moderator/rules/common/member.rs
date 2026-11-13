//! The member a trigger's subject concerns — a message's author, or the person
//! who joined —, for the conditions and actions that work on the member rather
//! than on a message, and so in the rules of more than one trigger (US-E31,
//! US-E32, US-F2, US-F3).
//!
//! A leaf that needs only this implements its trait for every subject that is
//! a `Member`; a leaf that needs the message implements it for `GroupMessage`
//! alone, and a trigger whose subject is not a message cannot list it.

use crate::domain::moderator::ports::{GroupMessage, MemberJoin, MessengerGroup, UserId};
use chrono::{DateTime, Utc};

/// What every subject tells about the member it concerns.
pub trait Member: Send + Sync {
    /// The group it happened in.
    fn group(&self) -> &MessengerGroup;
    /// The member, as the group knows them: the id a kick or a role change takes.
    fn member_id(&self) -> UserId;
    /// The name they chose for themselves, as other members see it.
    fn display_name(&self) -> &str;
    /// The full name of their profile, empty when they set none.
    fn full_name(&self) -> &str;
    /// When it happened: the time the rules are checked at.
    fn timestamp(&self) -> DateTime<Utc>;
}

impl Member for GroupMessage {
    fn group(&self) -> &MessengerGroup {
        &self.group
    }

    fn member_id(&self) -> UserId {
        self.author_id
    }

    fn display_name(&self) -> &str {
        &self.author_name
    }

    fn full_name(&self) -> &str {
        &self.author_full_name
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

impl Member for MemberJoin {
    fn group(&self) -> &MessengerGroup {
        &self.group
    }

    fn member_id(&self) -> UserId {
        self.member_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn full_name(&self) -> &str {
        &self.full_name
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}
