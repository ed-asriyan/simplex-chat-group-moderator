use super::ports::{
    BotDmReceiver, BotMessenger, DeleteAuthorMessages, DeleteObserverMessages, Err, GroupId,
    GroupInvitation, GroupOperations, ModerationAction, ModerationNotificationReceiver, UserId,
};
use crate::domain::bot_dm::ports::{Group, Message};
use async_trait::async_trait;
use std::sync::Arc;

fn lz_compress(s: &str) -> String {
    let data: Vec<u16> = s.encode_utf16().collect();
    lz_str::compress_to_encoded_uri_component(&data)
}
fn lz_decompress(s: &str) -> Option<String> {
    let data = lz_str::decompress_from_encoded_uri_component(s)?;
    String::from_utf16(&data).ok()
}

const MAX_MESSAGE_LENGTH: usize = 300;

const HELP: &str = "\
*How to use this bot:*

1. Invite me to your group as:
   • Moderator — to delete violating messages.
   • Admin — to also set violators as observers.
   • Owner — to also kick violators out of the group.
2. Once I join, use /groups to list your groups and open the visual rules editor.
3. I will automatically monitor the chat and take action according to your rules.

*If you want me to stop moderating a group, just kick me from it.*

*Commands:*
  /start   - Show welcome guide.
  /help    - Show this guide.
  /groups  - List and manage groups I moderate for you.
  /source  - View my source code.
  /issue   - Report a bug or unexpected moderation behaviour.
  /feature - Request a new moderation rule type or feature.

*Notifications:*
For each group you can turn moderation notifications on or off (use /groups to get the links). When enabled, I'll DM you whenever I take a moderation action.

*Dry mode:*
You can also enable dry mode for a group. In dry mode I run all checks and notify you about what I would moderate, but I don't actually delete anything or kick anyone. Use /groups to get the links.
";

const ISSUE_URL: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml";

const FEATURE_REQUEST_URL: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=feature-request.yml";

const START: &str = "\
Hi! I'm an *automated moderation* bot for SimpleX groups.

*What I can detect:*
• 🚫 *Banned words & exact phrases* (even obfuscated like b@d_w0rd)
• 🔗 *Links* (blocklist specific sites, allow only whitelist, or auto-allow Top 100 safe sites)
• ⏱ *Rate limits* (too many messages or repeated rule violations in a time window)
• 🌊 *Screen flooding* (long messages, empty/invisible text, line flood)
...and much more! This is just a glimpse of what I can do.

*What I can do to violators:*
• 🗑 *Moderate their messages*
• 👁 *Change role to observer*
• 🚪 *Kick them out of the group*

*How to get started:*
1. Invite me to your group as:
   • Moderator — if you only want me to 🗑 delete messages.
   • Admin — if you also want me to 👁 change roles to observer.
   • Owner — if you also want me to 🚪 kick users.
2. Once I join, use /groups to configure your moderation rules in the visual editor.

Use /help anytime for commands and extra features (like Dry Mode and notifications).

My code is open source and [available on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).
";

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
    GetGroups,
    SetNotifications { group_id: GroupId, enabled: bool },
    SetDryMode { group_id: GroupId, enabled: bool },
    Source,
    Issue,
    Feature,
    Unknown,
}

fn parse(message: &Message, base_url: &str) -> ParsedDm {
    let text = message.text.trim();
    let base = base_url.trim_end_matches('/');

    // Find the editor URL anywhere in the message (bare URL or inside a markdown link)
    if let Some(pos) = text.find(base) {
        let rest = &text[pos..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if let Some(fragment) = url.split_once('#').map(|(_, f)| f) {
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
    }

    if text.is_empty() {
        return ParsedDm::Unknown;
    }
    match text {
        "/start" => ParsedDm::Start,
        "/help" => ParsedDm::Help,
        "/source" => ParsedDm::Source,
        "/issue" => ParsedDm::Issue,
        "/feature" => ParsedDm::Feature,
        "/groups" => ParsedDm::GetGroups,
        _ if let Some(id_str) = text.strip_prefix("/notify_on_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetNotifications {
                group_id,
                enabled: true,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ if let Some(id_str) = text.strip_prefix("/notify_off_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetNotifications {
                group_id,
                enabled: false,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ if let Some(id_str) = text.strip_prefix("/dry_on_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetDryMode {
                group_id,
                enabled: true,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ if let Some(id_str) = text.strip_prefix("/dry_off_") => match id_str.trim().parse() {
            Ok(group_id) => ParsedDm::SetDryMode {
                group_id,
                enabled: false,
            },
            Err(_) => ParsedDm::Unknown,
        },
        _ => ParsedDm::Unknown,
    }
}

fn render_group(group: &Group, rules_url: &str) -> String {
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
        "*{}*\n[View and Edit Rules]({})\n{}\n{}",
        group.name, rules_url, notifications_command, dry_mode_command,
    )
}

#[async_trait]
impl BotDmReceiver for BotDmApplication {
    async fn handle_dm(&self, user_id: UserId, message: &Message) -> Result<(), Err> {
        match parse(message, &self.webeditor_base_url) {
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

            ParsedDm::GetGroups => {
                let groups = self.group_operator.get_groups(user_id).await?;
                if groups.is_empty() {
                    self.messenger.send_dm(&user_id, "You don't have any groups registered. Send me a group invite link to get started!")
                        .await?;
                } else {
                    for group in &groups {
                        let rules_url = match self
                            .group_operator
                            .get_rules_json(user_id, group.id)
                            .await?
                        {
                            Some(json) => {
                                let hash = lz_compress(&json);
                                format!(
                                    "{}#bot_id={}&rules={}",
                                    self.webeditor_base_url.trim_end_matches('/'),
                                    group.id,
                                    hash
                                )
                            }
                            None => String::new(),
                        };
                        self.messenger
                            .send_dm(&user_id, &render_group(group, &rules_url))
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
            ParsedDm::Issue => {
                let message = format!(
                    "Please report any bugs or unexpected moderation behaviour [here]({})",
                    ISSUE_URL
                );
                self.messenger.send_dm(&user_id, &message).await?;
            }
            ParsedDm::Feature => {
                let message = format!(
                    "Please request new moderation rule types or features [here]({})",
                    FEATURE_REQUEST_URL
                );
                self.messenger.send_dm(&user_id, &message).await?;
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
                    let rules_url = match self
                        .group_operator
                        .get_rules_json(user_id, group.id)
                        .await?
                    {
                        Some(json) => {
                            let hash = lz_compress(&json);
                            format!(
                                "{}#bot_id={}&rules={}",
                                self.webeditor_base_url.trim_end_matches('/'),
                                group.id,
                                hash
                            )
                        }
                        None => String::new(),
                    };
                    self.messenger
                        .send_dm(&user_id, &render_group(&group, &rules_url))
                        .await?;
                }
                Err(_) => {
                    self.messenger
                        .send_dm(
                            &user_id,
                            "Failed to join the group. Check if the invite link is correct and I have the moderator or owner role.",
                        )
                        .await?;
                }
            }
        } else {
            self.messenger
                .send_dm(
                    &user_id,
                    "I need to be added as a moderator (or owner) to join the group. Please update my permissions and send the invite again.",
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
        action: &ModerationAction,
        message: &str,
        reasons: &[String],
    ) -> Result<(), Err> {
        let message = if message.chars().count() > MAX_MESSAGE_LENGTH {
            format!(
                "{}...",
                message.chars().take(MAX_MESSAGE_LENGTH).collect::<String>()
            )
        } else {
            message.to_owned()
        };
        let action_text = match action {
            ModerationAction::ModerateMessage => {
                if group.dry_mode_enabled {
                    "🛡 I would moderate a message"
                } else {
                    "🛡 I moderated a message"
                }
            }
            ModerationAction::KickAuthor { delete_messages } => {
                if group.dry_mode_enabled {
                    match delete_messages {
                        DeleteAuthorMessages::AllMessages => {
                            "🛡 I would kick the author and delete all their messages"
                        }
                        DeleteAuthorMessages::TriggeredMessage => {
                            "🛡 I would kick the author and moderated their message"
                        }
                        DeleteAuthorMessages::None => "🛡 I would kick the author",
                    }
                } else {
                    match delete_messages {
                        DeleteAuthorMessages::AllMessages => {
                            "🛡 I kicked the author and deleted all their messages"
                        }
                        DeleteAuthorMessages::TriggeredMessage => {
                            "🛡 I kicked the author and moderated their message"
                        }
                        DeleteAuthorMessages::None => "🛡 I kicked the author",
                    }
                }
            }
            ModerationAction::SetAuthorObserver { delete_message } => {
                if group.dry_mode_enabled {
                    match delete_message {
                        DeleteObserverMessages::None => "🛡 I would set the author as observer",
                        DeleteObserverMessages::TriggeredMessage => {
                            "🛡 I would set the author as observer and moderate the message"
                        }
                    }
                } else {
                    match delete_message {
                        DeleteObserverMessages::None => "🛡 I set the author as observer",
                        DeleteObserverMessages::TriggeredMessage => {
                            "🛡 I set the author as observer and moderated the message"
                        }
                    }
                }
            }
        };
        let reasons = reasons.iter().map(|x| format!("• {}", x)).collect::<Vec<_>>().join("\n");
        let text = format!(
            "{} in *{}*!\n\n*The message:*\n{}\n\n*Reason:*\n{}",
            action_text, group.name, message, reasons,
        );
        self.messenger.send_dm(&user_id, &text).await
    }
}
