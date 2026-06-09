use super::ports::{
    BotDmReceiver, BotMessenger, Err, GroupId, GroupInvitation, GroupOperations,
    ModerationNotificationReceiver, UserId,
};
use crate::domain::bot_dm::ports::{Group, Message};
use async_trait::async_trait;
use const_format::formatcp;
use std::sync::Arc;

const HELP: &str = "\
How to use this bot:

1. Invite me to your group and make me a moderator so I have permission to delete messages.
2. Send me a list of words or phrases you want to block in that group.
   (Don't know what to block? Use /wordlists to get ready-made templates).
3. I will automatically monitor the chat and delete any message that triggers your list. I check not only direct matches, but also messages that try to obfuscate the blocked words (e.g. `s.p.a.m` or `spaaam` will match `spam`).

If you want me to stop moderating a group, just kick me from it.

Commands:
  /start     - Show this guide.
  /help      - Show this guide.
  /wordlists - Get links to ready-to-use lists of bad words.
  /source    - Link to my source code.
  /groups    - List and manage groups I moderate for you.
";

const START: &str = formatcp!(
    "Hi! Invite me to your group and grant me moderator permissions. \
    Then, you can send me a list of words or phrases to block (or use /wordlists for ready-made templates), \
    and I will automatically delete any messages containing them. \
    You can manage multiple groups with me.\n\n{}",
    HELP,
);

const WORD_LISTS: &str = "\
Here are some ready-to-use lists of words to block.
Open a link, copy the words you need, and reply to the group management message for your group. Each message you send will replace the whole list, so if you want to combine multiple lists, copy all the words into the single message.

*🔞 List of Dirty, Naughty, Obscene, and Otherwise Bad Words (multilanguage, ~400 en, ~1700 total)*
https://github.com/LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words

*☕ Google Profanity Words (multilanguage, ~1k en, ~1600 total)*
https://github.com/coffee-and-fun/google-profanity-words/tree/main/data

*💬 Comment Blocklist for WordPress (multilanguage, ~64k total)*
https://github.com/splorp/wordpress-comment-blocklist
";

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
    SetNotifications {
        group_id: GroupId,
        enabled: bool,
    },
    WordLists,
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
                "/wordlists" => ParsedDm::WordLists,
                "/groups" => ParsedDm::GetGroups,
                _ if let Some(id_str) = trimmed.strip_prefix("/getkeywords_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::GetBlockKeywords { group_id },
                        Err(_) => ParsedDm::Unknown,
                    }
                } // handled below
                _ if let Some(id_str) = trimmed.strip_prefix("/moderation_notifications_on_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetNotifications {
                            group_id,
                            enabled: true,
                        },
                        Err(_) => ParsedDm::Unknown,
                    }
                }
                _ if let Some(id_str) = trimmed.strip_prefix("/moderation_notifications_off_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetNotifications {
                            group_id,
                            enabled: false,
                        },
                        Err(_) => ParsedDm::Unknown,
                    }
                }
                _ => ParsedDm::Unknown,
            }
        }
    }
}

fn render_group(group: &Group) -> String {
    let notifications_command = if group.notifications_enabled {
        format!(
            "Disable moderation notifications: /moderation_notifications_off_{}",
            group.id
        )
    } else {
        format!(
            "Enable moderation notifications: /moderation_notifications_on_{}",
            group.id
        )
    };
    format!(
        "*{}* {}\n\
        View blocked words: /getkeywords_{}\n\
        {}\n\
        Reply to this message with a list of words or phrases to block in this group, each on a new line. Each message rewrites the whole list!",
        group.name,
        group_anchor(group),
        group.id,
        notifications_command,
    )
}

fn format_keywords(keywords: Vec<String>) -> Vec<String> {
    const MAX_KEYWORDS_PER_MESSAGE: usize = 1000;
    let mut result = vec![];
    let mut message = String::new();
    for keyword in keywords {
        if message.len() + keyword.len() + 1 > MAX_KEYWORDS_PER_MESSAGE {
            result.push(message.clone());
            message.clear();
        }
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(&keyword);
    }
    if !message.is_empty() {
        result.push(message);
    }
    result
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
                        for message in format_keywords(keywords) {
                            self.messenger.send_dm(&user_id, &message).await?;
                        }
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
            ParsedDm::SetNotifications { group_id, enabled } => {
                self.group_operator
                    .set_notifications(user_id, group_id, enabled)
                    .await?;
                let reply = if enabled {
                    "Moderation notifications enabled. I'll DM you whenever I delete a message in this group."
                } else {
                    "Moderation notifications disabled."
                };
                self.messenger.send_dm(&user_id, reply).await?;
            }
            ParsedDm::WordLists => {
                self.messenger.send_dm(&user_id, WORD_LISTS).await?;
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

#[async_trait]
impl ModerationNotificationReceiver for BotDmApplication {
    async fn send_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        message: &str,
    ) -> Result<(), Err> {
        let text = format!(
            "🛡 I moderated a message in *{}* {}:\n\n{}",
            group.name,
            group_anchor(group),
            message,
        );
        self.messenger.send_dm(&user_id, &text).await
    }
}
