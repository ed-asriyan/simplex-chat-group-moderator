use async_trait::async_trait;
use std::error::Error;

pub type MessengerGroupId = i64;
pub type GroupId = i64;
pub type MessageId = i64;
pub type UserId = i64;

pub type Err = Box<dyn Error + Send + Sync>;

pub struct Group {
    pub id: GroupId,
    pub owner_id: UserId,
    pub name: String,
    pub notifications_enabled: bool,
    pub dry_mode_enabled: bool,
}

pub struct MessengerGroup {
    pub id: MessengerGroupId,
    pub name: String,
}

pub struct GroupInvitation {
    pub group: MessengerGroup,
    pub is_moderator: bool,
}

pub struct GroupMessage {
    pub group: MessengerGroup,
    pub message_id: MessageId,
    pub author_id: UserId,
    pub text: String,
}

/// Inbound port: the moderator bounded context's use cases.
#[async_trait]
pub trait ModerationEngine: Send + Sync {
    async fn process_group_message(&self, group_message: GroupMessage) -> Result<(), Err>;

    async fn set_keywords(
        &self,
        user_id: UserId,
        group_id: GroupId,
        keywords: Vec<String>,
    ) -> Result<(), Err>;

    async fn try_join_group(
        &self,
        owner_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<GroupId, Err>;

    async fn get_keywords(&self, user_id: UserId, group_id: GroupId) -> Result<Vec<String>, Err>;

    async fn remove_group(&self, messenger_group_id: MessengerGroupId) -> Result<(), Err>;

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err>;

    async fn set_notifications(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err>;

    async fn set_dry_mode(
        &self,
        user_id: UserId,
        group_id: GroupId,
        enabled: bool,
    ) -> Result<(), Err>;
}

/// Outbound port: notify a group owner that a message was moderated.
#[async_trait]
pub trait ModerationNotifier: Send + Sync {
    async fn notify_moderated_message(
        &self,
        user_id: UserId,
        group: &Group,
        message: &str,
        phrase: &str,
    ) -> Result<(), Err>;
}

/// Outbound port: actions the moderator performs in a group.
#[async_trait]
pub trait GroupModerator: Send + Sync {
    async fn delete_message(&self, group_id: &GroupId, message_id: &MessageId) -> Result<(), Err>;

    async fn join_group(
        &self,
        messenger_group_id: MessengerGroupId,
    ) -> Result<MessengerGroupId, Err>;
}

/// Outbound port: persistence for moderator state.
#[async_trait]
pub trait ModerationRepository: Send + Sync {
    /// Register a new group and return the generated `group_id`.
    async fn save_owner(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
        owner_id: &UserId,
    ) -> Result<GroupId, Err>;

    async fn get_owner_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<UserId>, Err>;

    async fn get_groups_by_owner_id(&self, owner_id: &UserId) -> Result<Vec<Group>, Err>;

    async fn get_owner_by_id(&self, group_id: &GroupId) -> Result<Option<UserId>, Err>;

    async fn save_keywords(&self, group_id: &GroupId, keywords: Vec<String>) -> Result<(), Err>;

    async fn set_group_name(
        &self,
        messenger_group_id: &MessengerGroupId,
        name: &str,
    ) -> Result<(), Err>;

    async fn get_keywords(&self, group_id: &GroupId) -> Result<Vec<String>, Err>;

    async fn get_keywords_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Vec<String>, Err>;

    async fn delete_group_data(&self, messenger_group_id: &MessengerGroupId) -> Result<(), Err>;

    async fn get_group_by_messenger_id(
        &self,
        messenger_group_id: &MessengerGroupId,
    ) -> Result<Option<Group>, Err>;

    async fn set_notifications_enabled(&self, group_id: &GroupId, enabled: bool)
    -> Result<(), Err>;

    async fn set_dry_mode_enabled(&self, group_id: &GroupId, enabled: bool) -> Result<(), Err>;
}
