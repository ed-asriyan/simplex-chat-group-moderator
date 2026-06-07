use super::ports::{
    BotDmReceiver, BotMessenger, Err, GroupId, GroupInvitation, GroupOperations, UserId,
};
use crate::domain::bot_dm::ports::{Group, Message};
use async_trait::async_trait;
use const_format::formatcp;
use std::sync::Arc;

const HELP: &str = "\
How to use this bot:

1. Invite me to your group and make me a moderator so I have permission to delete messages.
2. Send me a list of words or phrases you want to block in that group.
3. I will automatically monitor the chat and delete any message that triggers your list.

You can update your blocked words or remove me from the group at any time.

Deletion criteria:
- The message contains at least one of your blocked words or phrases (case-insensitive).

Commands:
  /start, /help - Show this guide.
  /source - Link to my source code.
  /groups - List and manage groups I moderate for you.
  /
";

const START: &str = formatcp!(
    "Hi! Invite me to your group and grant me moderator permissions. Then, you can send me a list of words or phrases to block, and I will automatically delete any messages containing them. You can manage multiple groups with me.\n\n{}",
    HELP,
);

fn group_anchor(group: &Group) -> String {
    format!("#{}", group.id)
}

fn find_group_by_anchor<'a>(text: &str) -> Option<GroupId> {
    text.split_whitespace()
        .find_map(|word| word.strip_prefix('#')?.parse().ok())
}

pub struct BotDmApplication {
    messenger: Arc<dyn BotMessenger>,
    group_operator: Arc<dyn GroupOperations>,
}

impl BotDmApplication {
    pub fn new(messenger: Arc<dyn BotMessenger>, group_operator: Arc<dyn GroupOperations>) -> Self {
        Self {
            messenger,
            group_operator,
        }
    }
}

enum ParsedDm {
    Start,
    Help,
    SetBlockKeywords {
        group_id: GroupId,
        keywords: Vec<String>,
    },
    GetBlockKeywords {
        group_id: GroupId,
    },
    GetGroups,
    Source,
    Unknown,
}

fn parse_keywords(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse(message: &Message) -> ParsedDm {
    if let Some(group_id) = message
        .reply_to_message
        .as_ref()
        .and_then(|reply| find_group_by_anchor(reply))
    {
        ParsedDm::SetBlockKeywords {
            group_id,
            keywords: parse_keywords(&message.text),
        }
    } else {
        let trimmed = message.text.trim();
        if trimmed.is_empty() {
            ParsedDm::Unknown
        } else {
            match trimmed {
                "/start" => ParsedDm::Start,
                "/help" => ParsedDm::Help,
                "/source" => ParsedDm::Source,
                "/groups" => ParsedDm::GetGroups,
                _ if let Some(id_str) = trimmed.strip_prefix("/getkeywords_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::GetBlockKeywords { group_id },
                        Err(_) => ParsedDm::Unknown,
                    }
                } // handled below
                _ => ParsedDm::Unknown,
            }
        }
    }
}

fn render_group(group: &Group) -> String {
    format!(
        "*{}* {}\n\
        View blocked words: /getkeywords_{}\n\
        Reply to this message with a list of words or phrases to block in this group, each on a new line.",
        group.name,
        group_anchor(group),
        group.id,
    )
}

#[async_trait]
impl BotDmReceiver for BotDmApplication {
    async fn handle_dm(&self, user_id: UserId, message: &Message) -> Result<(), Err> {
        match parse(message) {
            ParsedDm::Start => {
                self.messenger.send_dm(&user_id, START).await?;
            }
            ParsedDm::Help => {
                self.messenger.send_dm(&user_id, HELP).await?;
            }
            ParsedDm::SetBlockKeywords { group_id, keywords } => {
                self.group_operator
                    .set_keywords(user_id, group_id, keywords)
                    .await?;
                self.messenger
                    .send_dm(&user_id, "Keywords updated successfully.")
                    .await?;
            }
            ParsedDm::GetBlockKeywords { group_id } => {
                let keywords = self.group_operator.get_keywords(user_id, group_id).await?;
                match keywords {
                    Some(keywords) if keywords.is_empty() => {
                        self.messenger
                            .send_dm(&user_id, "No blocked keywords set for this group.")
                            .await?;
                    }
                    Some(keywords) => {
                        self.messenger
                            .send_dm(&user_id, &keywords.join("\n"))
                            .await?;
                    }
                    None => {
                        self.messenger
                            .send_dm(&user_id, "Group not found or not managed by you.")
                            .await?;
                    }
                }
            }
            ParsedDm::GetGroups => {
                let groups = self.group_operator.get_groups(user_id).await?;
                if groups.is_empty() {
                    self.messenger.send_dm(&user_id, "You don't have any groups registered. Send me a group invite link to get started!")
                        .await?;
                } else {
                    for group in &groups {
                        self.messenger
                            .send_dm(&user_id, &render_group(group))
                            .await?;
                    }

                    self.messenger
                        .send_dm(&user_id, "To delete one, just kick me from the group.")
                        .await?;
                }
            }
            ParsedDm::Source => {
                self.messenger
                    .send_dm(
                        &user_id,
                        "https://github.com/ed-asriyan/simplex-chat-group-moderator",
                    )
                    .await?;
            }
            ParsedDm::Unknown => {
                self.messenger
                    .send_dm(
                        &user_id,
                        "Unknown command. Send /help to see what I can do.",
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_group_invitation(
        &self,
        user_id: UserId,
        invitation: &GroupInvitation,
    ) -> Result<(), Err> {
        if invitation.is_moderator {
            match self
                .group_operator
                .try_join_group(user_id, invitation)
                .await
            {
                Ok(group) => {
                    self.messenger
                        .send_dm(&user_id, "Joined the group successfully!")
                        .await?;
                    self.messenger
                        .send_dm(&user_id, &render_group(&group))
                        .await?;
                }
                Err(_) => {
                    self.messenger
                        .send_dm(
                            &user_id,
                            "Failed to join the group. Check if the invite link is correct and I have the moderator role.",
                        )
                        .await?;
                }
            }
        } else {
            self.messenger
                .send_dm(
                    &user_id,
                    "I need to be added as a moderator to join the group. Please update my permissions and send the invite again.",
                )
                .await?;
        }
        Ok(())
    }
}
