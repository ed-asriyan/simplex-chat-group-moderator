use async_trait::async_trait;
use std::error::Error;

pub type GroupId = i64;
pub type MessageId = i64;
pub type UserId = i64;

pub type Err = Box<dyn Error + Send + Sync>;

pub type JoinError = Box<dyn Error + Send + Sync>;

#[derive(Debug)]
pub struct Message {
    pub text: String,
    pub reply_to_message: Option<String>,
}

pub struct Group {
    pub id: GroupId,
    pub name: String,
}

pub struct GroupInvitation {
    pub group: Group,
    pub is_moderator: bool,
}

/// Inbound port: entry point for direct messages addressed to the bot.
#[async_trait]
pub trait BotDmReceiver: Send + Sync {
    async fn handle_dm(&self, user_id: UserId, message: &Message) -> Result<(), Err>;

    async fn handle_group_invitation(
        &self,
        user_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<(), Err>;
}

/// Outbound port: send direct messages back to a user.
#[async_trait]
pub trait BotMessenger: Send + Sync {
    async fn send_dm(&self, user_id: &UserId, text: &str) -> Result<(), Err>;
}

/// Outbound port: group lifecycle operations the bot triggers from DMs.
#[async_trait]
pub trait GroupOperations: Send + Sync {
    /// Attempt to join the group identified by the given invitation
    async fn try_join_group(
        &self,
        user_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<Group, JoinError>;

    async fn set_keywords(
        &self,
        user_id: UserId,
        group_id: GroupId,
        keywords: Vec<String>,
    ) -> Result<(), Err>;

    async fn get_groups(&self, user_id: UserId) -> Result<Vec<Group>, Err>;

    async fn get_keywords(
        &self,
        user_id: UserId,
        group_id: GroupId,
    ) -> Result<Option<Vec<String>>, Err>;
}
