use super::ports::{
    BotDmReceiver, BotMessenger, Err, GroupId, GroupInvitation, GroupOperations,
    ModerationNotificationReceiver, UserId,
};
use crate::domain::bot_dm::ports::{Group, Message};
use async_trait::async_trait;
use const_format::formatcp;
use std::sync::Arc;

fn lz_compress(s: &str) -> String {
    let data: Vec<u16> = s.encode_utf16().collect();
    lz_str::compress_to_encoded_uri_component(&data)
}
fn lz_decompress(s: &str) -> Option<String> {
    let data = lz_str::decompress_from_encoded_uri_component(s)?;
    String::from_utf16(&data).ok()
}

const HELP: &str = "\
How to use this bot:

1. Invite me to your group and make me a moderator so I have permission to delete messages.
2. Use /groups to list your groups. For each group, tap the rules link to view and edit its moderation rules.
3. I will automatically monitor the chat and delete any message that violates the rules.

If you want me to stop moderating a group, just kick me from it.

Commands:
  /start  - Show this guide.
  /help   - Show this guide.
  /groups - List and manage groups I moderate for you.
  /source - Link to my source code.

For each group you can also turn moderation notifications on or off (use /groups to get the links). When enabled, I'll DM you whenever I delete a message.

You can also enable dry mode for a group. In dry mode I run all checks and notify you about what I would delete, but I don't actually delete anything. Use /groups to get the links.
";

const START: &str = formatcp!(
    "Hi! Invite me to your group and grant me moderator permissions. \
    Then use /groups to configure moderation rules for it \
    — I support keyword blocking, link blacklists, link whitelists, and more. \
    You can manage multiple groups with me.\n\n{}",
    HELP,
);

pub struct BotDmApplication {
    messenger: Arc<dyn BotMessenger>,
    group_operator: Arc<dyn GroupOperations>,
    webeditor_base_url: String,
}

impl BotDmApplication {
    pub fn new(
        messenger: Arc<dyn BotMessenger>,
        group_operator: Arc<dyn GroupOperations>,
        webeditor_base_url: String,
    ) -> Self {
        Self {
            messenger,
            group_operator,
            webeditor_base_url,
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
    Source,
    Unknown,
}

fn parse(message: &Message) -> ParsedDm {
    let trimmed = message.text.trim();

    // Detect a webeditor URL: extract bot_id and rules from the hash fragment
    if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && let Some(fragment) = trimmed.split_once('#').map(|(_, f)| f)
    {
        let mut bot_id: Option<GroupId> = None;
        let mut rules_hash: Option<String> = None;
        for param in fragment.split('&') {
            if let Some(v) = param.strip_prefix("bot_id=") {
                bot_id = v.parse().ok();
            } else if let Some(v) = param.strip_prefix("rules=") {
                rules_hash = Some(v.to_string());
            }
        }
        if let (Some(group_id), Some(hash)) = (bot_id, rules_hash) {
            return ParsedDm::SetRules {
                group_id,
                yaml: hash,
            };
        }
    }

    if trimmed.is_empty() {
        return ParsedDm::Unknown;
    }
    match trimmed {
        "/start" => ParsedDm::Start,
        "/help" => ParsedDm::Help,
        "/source" => ParsedDm::Source,
        "/groups" => ParsedDm::GetGroups,
        _ if let Some(id_str) = trimmed.strip_prefix("/rules_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::GetRules { group_id },
            Err(_) => ParsedDm::Unknown,
        }, // handled below
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
        _ if let Some(id_str) = trimmed.strip_prefix("/dry_on_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetDryMode {
                group_id,
                enabled: true,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ if let Some(id_str) = trimmed.strip_prefix("/dry_off_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetDryMode {
                group_id,
                enabled: false,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ => ParsedDm::Unknown,
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
        "*{}*
View & Edit Rules: /rules_{}
{}
{}\
",
        group.name, group.id, notifications_command, dry_mode_command,
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
            ParsedDm::SetRules {
                group_id,
                yaml: rules_hash,
            } => {
                let result = match lz_decompress(&rules_hash) {
                    Some(json) => {
                        self.group_operator
                            .set_rules_json(user_id, group_id, &json)
                            .await
                    }
                    None => Err("Could not decompress rules from the URL. \
                        Open the editor link again, make your changes, \
                        and send back the updated URL."
                        .into()),
                };
                match result {
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
                match self
                    .group_operator
                    .get_rules_json(user_id, group_id)
                    .await?
                {
                    Some(json) => {
                        let hash = lz_compress(&json);
                        let editor_url = format!(
                            "{}#bot_id={}&rules={}",
                            self.webeditor_base_url.trim_end_matches('/'),
                            group_id,
                            hash
                        );
                        let text = format!(
                            "Open the link to edit rules in the visual editor and follow instructions: {editor_url}",
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
