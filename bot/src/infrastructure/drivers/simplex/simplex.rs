use super::consts::{BOT_PHOTO, TTL};
use async_stream::stream;
use futures::TryStreamExt as _;
use futures::stream::Stream;
use simploxide_client::events::Event;
use simploxide_client::prelude::ApiSendMessages;
use simploxide_client::types::{
    CIContent, ChatBotCommand, ChatInfo, ChatPeerType, ChatRef, ChatType, ComposedMessage,
    FeatureAllowed, GroupMemberRole, MsgContent, SimplePreference,
};
use simploxide_client::{
    Client, ClientApi, EventStream,
    types::{Preferences, Profile},
};
use std::error::Error;
use std::vec;

pub type UserId = i64;
pub type MessageId = i64;
pub type GroupId = i64;

pub enum SimplexEvent {
    Message {
        user_id: UserId,
        text: String,
        message_id: MessageId,
        reply_message_text: Option<String>,
    },
    GroupMessage {
        group_id: GroupId,
        author_id: UserId,
        message_id: MessageId,
        text: String,
    },
    Connected {
        user_id: UserId,
    },
    Disconnected {
        user_id: UserId,
    },
    RemovedFromGroup {
        group_id: GroupId,
    },
    GroupInvitation {
        user_id: UserId,
        group_id: GroupId,
        group_name: String,
        is_moderator: bool,
    },
}
pub struct SimpleXConfig {
    pub simplex_uri: String,
    pub display_name: String,
    pub full_name: String,
    pub short_description: String,
}

#[derive(Clone)]
pub struct SimplexDriver {
    client: Client,
}

impl SimplexDriver {
    pub async fn get_bot_address(&self) -> Result<String, Box<dyn Error>> {
        let user = self.client.show_active_user().await?;
        let address = self.client.api_show_my_address(user.user_id).await?;
        let address = address
            .contact_link
            .conn_link_contact
            .conn_short_link
            .clone()
            .unwrap_or(
                address
                    .contact_link
                    .conn_link_contact
                    .conn_full_link
                    .clone(),
            );
        Ok(address)
    }

    pub async fn get_or_create_bot_address(&self) -> Result<String, Box<dyn Error>> {
        let address = self.get_bot_address().await;
        if let Err(_) = address {
            let user = self.client.show_active_user().await?;
            self.client.api_create_my_address(user.user_id).await?;
            self.get_bot_address().await
        } else {
            address
        }
    }

    pub async fn send_message_text(
        &self,
        user_id: UserId,
        text: String,
        reply_id: Option<i64>,
    ) -> Result<i64, Box<dyn Error>> {
        let composed_messages = ComposedMessage::builder()
            .msg_content(MsgContent::Text {
                text,
                undocumented: Default::default(),
            })
            .mentions(Default::default())
            .maybe_quoted_item_id(
                if let Some(id) = reply_id
                    && id > 0
                {
                    Some(id)
                } else {
                    None
                },
            )
            .build();

        let result = self
            .client
            .api_send_messages(
                ApiSendMessages::builder()
                    .send_ref(
                        ChatRef::builder()
                            .chat_type(ChatType::Direct)
                            .chat_id(user_id)
                            .build(),
                    )
                    .live_message(false)
                    .composed_messages(vec![composed_messages])
                    .build(),
            )
            .await?;

        let message_id = result
            .chat_items
            .first()
            .ok_or("No message ID returned")?
            .chat_item
            .meta
            .item_id;

        Ok(message_id)
    }

    /// Join a group/contact via a SimpleX invite link.
    /// Returns the joined group's id.
    pub async fn join_group(&self, group_id: GroupId) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client.api_join_group(group_id).await?;
        Ok(())
    }

    /// Delete a message in a group chat.
    pub async fn moderate_group_message(
        &self,
        group_id: GroupId,
        message_id: MessageId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client
            .send_raw(format!("/_delete member item #{} {}", group_id, message_id))
            .await?;
        Ok(())
    }

    pub async fn new(
        config: SimpleXConfig,
    ) -> Result<(Self, impl Stream<Item = SimplexEvent>), Box<dyn Error>> {
        let (client, event_stream) = simploxide_client::connect(&config.simplex_uri).await?;
        let user = client.show_active_user().await?;
        client
            .api_update_profile(
                user.user_id,
                Profile::builder()
                    .display_name(config.display_name.clone())
                    .full_name(config.full_name.clone())
                    .short_descr(config.short_description.clone())
                    .peer_type(ChatPeerType::Bot)
                    .maybe_image(Some(BOT_PHOTO.to_string()))
                    .preferences(
                        Preferences::builder()
                            .full_delete(
                                SimplePreference::builder()
                                    .allow(FeatureAllowed::Yes)
                                    .build(),
                            )
                            .maybe_calls(Some(
                                SimplePreference::builder()
                                    .allow(FeatureAllowed::No)
                                    .build(),
                            ))
                            .maybe_reactions(Some(
                                SimplePreference::builder()
                                    .allow(FeatureAllowed::Yes)
                                    .build(),
                            ))
                            .maybe_files(Some(
                                SimplePreference::builder()
                                    .allow(FeatureAllowed::No)
                                    .build(),
                            ))
                            .maybe_voice(Some(
                                SimplePreference::builder()
                                    .allow(FeatureAllowed::No)
                                    .build(),
                            ))
                            .maybe_commands(Some(vec![
                                ChatBotCommand::Command {
                                    keyword: "start".to_string(),
                                    label: "Start interaction".to_string(),
                                    params: None,
                                    undocumented: Default::default(),
                                },
                                ChatBotCommand::Command {
                                    keyword: "groups".to_string(),
                                    label: "List of groups".to_string(),
                                    params: None,
                                    undocumented: Default::default(),
                                },
                                ChatBotCommand::Command {
                                    keyword: "wordlists".to_string(),
                                    label: "Ready-to-use lists of bad words".to_string(),
                                    params: None,
                                    undocumented: Default::default(),
                                },
                                ChatBotCommand::Command {
                                    keyword: "help".to_string(),
                                    label: "Show help information".to_string(),
                                    params: None,
                                    undocumented: Default::default(),
                                },
                                ChatBotCommand::Command {
                                    keyword: "source".to_string(),
                                    label: "Show source code".to_string(),
                                    params: None,
                                    undocumented: Default::default(),
                                },
                            ]))
                            .build(),
                    )
                    .build(),
            )
            .await?;

        client
            .send_raw(format!("/_ttl {} {}", user.user_id, TTL))
            .await?;

        Ok((
            SimplexDriver {
                client: client.clone(),
            },
            messages_stream(client, event_stream, user.user_id),
        ))
    }
}

async fn handle_event(
    client: &Client,
    event: &Event,
    user_id: UserId,
) -> Result<Vec<SimplexEvent>, Box<dyn Error + Send + Sync>> {
    match event {
        Event::GroupDeleted(group) => Ok(vec![SimplexEvent::RemovedFromGroup {
            group_id: group.group_info.group_id,
        }]),
        Event::ContactConnected(connected) => Ok(vec![SimplexEvent::Connected {
            user_id: connected.contact.contact_id,
        }]),
        Event::ReceivedContactRequest(req) => {
            client
                .api_accept_contact(req.contact_request.contact_request_id)
                .await?;
            Ok(vec![])
        }
        Event::DeletedMemberUser(user, ..) => {
            if user.user.user_id == user_id {
                Ok(vec![SimplexEvent::RemovedFromGroup {
                    group_id: user.member.group_id,
                }])
            } else {
                Ok(vec![])
            }
        }
        Event::NewChatItems(new_msgs) => Ok(new_msgs
            .chat_items
            .clone()
            .into_iter()
            .filter_map(|chat_item| match &chat_item.chat_info {
                ChatInfo::Direct { contact, .. } => {
                    if let CIContent::RcvMsgContent { msg_content, .. } =
                        &chat_item.chat_item.content
                        && let MsgContent::Text { text, .. } = &msg_content
                    {
                        Some(SimplexEvent::Message {
                            user_id: contact.contact_id,
                            message_id: chat_item.chat_item.meta.item_id,
                            text: text.clone(),
                            reply_message_text: chat_item.chat_item.quoted_item.and_then(|item| {
                                match item.content {
                                    MsgContent::Text { text, .. } => Some(text),
                                    _ => None,
                                }
                            }),
                        })
                    } else if let CIContent::RcvGroupInvitation {
                        group_invitation,
                        member_role,
                        ..
                    } = &chat_item.chat_item.content
                    {
                        Some(SimplexEvent::GroupInvitation {
                            user_id: contact.contact_id,
                            group_id: group_invitation.group_id,
                            group_name: group_invitation.group_profile.display_name.clone(),
                            is_moderator: matches!(member_role, GroupMemberRole::Moderator),
                        })
                    } else {
                        None
                    }
                }
                ChatInfo::Group { group_info, .. } => {
                    if let CIContent::RcvMsgContent { msg_content, .. } =
                        &chat_item.chat_item.content
                        && let MsgContent::Text { text, .. } = &msg_content
                    {
                        Some(SimplexEvent::GroupMessage {
                            group_id: group_info.group_id,
                            author_id: group_info.group_id,
                            message_id: chat_item.chat_item.meta.item_id,
                            text: text.clone(),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect::<Vec<SimplexEvent>>()),
        Event::NewMemberContactReceivedInv(req) => {
            client
                .send_raw(format!(
                    "/_accept member contact @{}",
                    req.contact.contact_id
                ))
                .await?;
            Ok(vec![])
        }
        Event::ContactDeletedByContact(user, ..) => Ok(vec![SimplexEvent::Disconnected {
            user_id: user.contact.contact_id,
        }]),
        _ => Ok(vec![]),
    }
}

fn messages_stream(
    client: Client,
    mut event_stream: EventStream,
    user_id: UserId,
) -> impl Stream<Item = SimplexEvent> {
    stream! {
        while let Ok(Some(event)) = event_stream.try_next().await {
            if let Ok(msgs) = handle_event(&client, &event, user_id).await {
                for msg in msgs {
                    yield msg;
                }
            }
        }
    }
}
