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

For each group you can also turn moderation notifications on or off (use /groups to get the links). When enabled, I'll DM you whenever I delete a message.

You can also enable dry mode for a group. In dry mode I run all checks and notify you about what I would delete, but I don't actually delete anything. Use /groups to get the links.
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
    SetRules { group_id: GroupId, yaml: String },
    GetRules { group_id: GroupId },
    GetGroups,
    SetNotifications { group_id: GroupId, enabled: bool },
    SetDryMode { group_id: GroupId, enabled: bool },
    WordLists,
    Source,
    Unknown,
}

fn parse(message: &Message) -> ParsedDm {
    if let Some(group_id) = message
        .reply_to_message
        .as_ref()
        .and_then(|reply| find_group_by_anchor(reply))
    {
        ParsedDm::SetRules {
            group_id,
            yaml: message.text.clone(),
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
                _ if let Some(id_str) = trimmed.strip_prefix("/rules_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::GetRules { group_id },
                        Err(_) => ParsedDm::Unknown,
                    }
                } // handled below
                _ if let Some(id_str) = trimmed.strip_prefix("/notify_on_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetNotifications {
                            group_id,
                            enabled: true,
                        },
                        Err(_) => ParsedDm::Unknown,
                    }
                }
                _ if let Some(id_str) = trimmed.strip_prefix("/notify_off_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetNotifications {
                            group_id,
                            enabled: false,
                        },
                        Err(_) => ParsedDm::Unknown,
                    }
                }
                _ if let Some(id_str) = trimmed.strip_prefix("/dry_on_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetDryMode {
                            group_id,
                            enabled: true,
                        },
                        Err(_) => ParsedDm::Unknown,
                    }
                }
                _ if let Some(id_str) = trimmed.strip_prefix("/dry_off_") => {
                    match id_str.trim().parse() {
                        Ok(group_id) => ParsedDm::SetDryMode {
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
        format!("Stop notifying me on delete: /notify_off_{}", group.id)
    } else {
        format!("Notify me on delete: /notify_on_{}", group.id)
    };
    let dry_mode_command = if group.dry_mode_enabled {
        format!("Disable dry mode: /dry_off_{}", group.id)
    } else {
        format!("Enable dry mode: /dry_on_{}", group.id)
    };
    format!(
        "*{}* {}
        View & Edit Rules: /rules_{}
        {}
        {}\
        ",
        group.name,
        group_anchor(group),
        group.id,
        notifications_command,
        dry_mode_command,
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
            ParsedDm::SetRules { group_id, yaml } => {
                match self
                    .group_operator
                    .set_rules_yaml(user_id, group_id, &yaml)
                    .await
                {
                    Ok(()) => {
                        self.messenger
                            .send_dm(&user_id, "Rules updated successfully.")
                            .await?;
                    }
                    Err(e) => {
                        self.messenger
                            .send_dm(&user_id, &format!("Failed to update rules: {}", e))
                            .await?;
                    }
                }
            }
            ParsedDm::GetRules { group_id } => {
                let yaml_opt = self
                    .group_operator
                    .get_rules_yaml(user_id, group_id)
                    .await?;
                match yaml_opt {
                    Some(yaml) => {
                        self.messenger.send_dm(&user_id, &yaml).await?;

                        let text = format!(
                            "Group Rules {}

To change the rules:
1. Copy the YAML text from the message above.
2. Edit it.
3. Reply back to *this message* with your new YAML text to apply the changes.

Docs: https://github.com/simplex-chat/group-moderator/blob/master/docs/RULES.md",
                            group_anchor(&Group {
                                id: group_id,
                                name: String::new(),
                                notifications_enabled: false,
                                dry_mode_enabled: false
                            }),
                        );
                        self.messenger.send_dm(&user_id, &text).await?;
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
                        .send_dm(
                            &user_id,
                            "To set the blocked words for a group, reply to that group's \
                             message above with a list of words or phrases, each on a new line. \
                             Each reply rewrites the whole list!\n\n\
                             If you want me to stop moderating a group, just kick me from it.",
                        )
                        .await?;
                }
            }
            ParsedDm::SetNotifications { group_id, enabled } => {
                match self
                    .group_operator
                    .set_notifications(user_id, group_id, enabled)
                    .await
                {
                    Ok(()) => {
                        let reply = if enabled {
                            "Moderation notifications enabled. I'll DM you whenever I delete a message in this group."
                        } else {
                            "Moderation notifications disabled."
                        };
                        self.messenger.send_dm(&user_id, reply).await?;
                    }
                    Err(_) => {
                        self.messenger
                            .send_dm(&user_id, "Group not found or not managed by you.")
                            .await?;
                    }
                }
            }
            ParsedDm::SetDryMode { group_id, enabled } => {
                match self
                    .group_operator
                    .set_dry_mode(user_id, group_id, enabled)
                    .await
                {
                    Ok(()) => {
                        let reply = if enabled {
                            "Dry mode enabled. I'll run all checks and notify you, but I won't actually delete any messages in this group. Notifications have been turned on."
                        } else {
                            "Dry mode disabled. I'll moderate messages in this group again."
                        };
                        self.messenger.send_dm(&user_id, reply).await?;
                    }
                    Err(_) => {
                        self.messenger
                            .send_dm(&user_id, "Group not found or not managed by you.")
                            .await?;
                    }
                }
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
        phrase: &str,
    ) -> Result<(), Err> {
        let text = format!(
            "{} a message in *{}*!\n\n*The message:*\n{}\n\n*Phrase found:*\n{}",
            if group.dry_mode_enabled {
                "🛡 I would moderate"
            } else {
                "🛡 I moderated"
            },
            group.name,
            message,
            phrase,
        );
        self.messenger.send_dm(&user_id, &text).await
    }
}
