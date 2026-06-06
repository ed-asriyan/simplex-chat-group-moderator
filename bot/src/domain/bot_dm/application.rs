use super::ports::{
    BotDmReceiver, BotMessenger, Err, GroupId, GroupInvitation, GroupOperations, UserId,
};
use crate::domain::bot_dm::ports::Group;
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
  /start, /help              Show this guide.
  /source                    Link to my source code.
  /groups                    List and manage groups I moderate for you.
";

const START: &str = formatcp!(
    "Hi! Invite me to your group and grant me moderator permissions. Then, you can send me a list of words or phrases to block, and I will automatically delete any messages containing them. You can manage multiple groups with me.\n\n{}",
    HELP,
);

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

fn parse(text: &str) -> ParsedDm {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return ParsedDm::Unknown;
    }

    match trimmed {
        "/start" => return ParsedDm::Start,
        "/help" => return ParsedDm::Help,
        "/source" => return ParsedDm::Source,
        "/groups" => return ParsedDm::GetGroups,
        _ => {}
    }

    if let Some(rest) = trimmed.strip_prefix("/setkeywords_") {
        let (id_str, kw_str) = match rest.split_once(char::is_whitespace) {
            Some((id, kw)) => (id, kw),
            None => (rest, ""),
        };
        let group_id: GroupId = match id_str.parse() {
            Ok(g) => g,
            Err(_) => return ParsedDm::Unknown,
        };
        let keywords: Vec<String> = kw_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if keywords.is_empty() {
            return ParsedDm::Unknown;
        }
        return ParsedDm::SetBlockKeywords { group_id, keywords };
    }

    if let Some(id_str) = trimmed.strip_prefix("/getkeywords_") {
        let group_id: GroupId = match id_str.trim().parse() {
            Ok(g) => g,
            Err(_) => return ParsedDm::Unknown,
        };
        return ParsedDm::GetBlockKeywords { group_id };
    }

    ParsedDm::Unknown
}

fn render_group(group: &Group) -> String {
    format!(
        "*{}*\n\
        View blocked words: /getkeywords_{}\n\
        Set blocked words: /setkeywords_{} <word1, word2, ...>",
        group.name, group.id, group.id
    )
}

#[async_trait]
impl BotDmReceiver for BotDmApplication {
    async fn handle_dm(&self, user_id: UserId, text: String) -> Result<(), Err> {
        match parse(&text) {
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
                            .send_dm(&user_id, "Here are the blocked words and phrases for this group. You can copy the message, edit the list, and send it back.")
                            .await?;
                        self.messenger
                            .send_dm(
                                &user_id,
                                &format!("/setkeywords_{} {}", group_id, keywords.join(", ")),
                            )
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
                    let items = groups
                        .iter()
                        .map(render_group)
                        .collect::<Vec<String>>()
                        .join("\n\n");
                    self.messenger
                        .send_dm(
                            &user_id,
                            &format!(
                                "Your groups:\n\n{}\n\nTo delete one, just remove me from your group",
                                items,
                            ),
                        )
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
