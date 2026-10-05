use super::ports::{
    BotDmReceiver, BotMessenger, Err, GroupId, GroupInvitation, GroupOperations, ModerationAction,
    ModerationNotificationReceiver, UserId,
};
use crate::domain::bot_dm::ports::{Group, Message};
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(test)]
mod tests;

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
2. Once I join, use /groups to list your groups and open the visual editor.
3. I will automatically monitor the chat and take action according to your rules.

*If you want me to stop moderating a group, just kick me from it.*

*Commands:*
  /start   - Show welcome guide.
  /help    - Show this guide.
  /groups  - List and manage groups I moderate for you.
  /source  - View my source code.
  /issue   - Report a bug or unexpected moderation behaviour.
  /feature - Request a new moderation rule type or feature.

*Modes:*
Each group has a mode, set in the same editor as its rules (use /groups to get the link):
  • Notifications — I take action and DM you every time I do.
  • Silent — I take action without telling you.
  • Dry — I run all checks and DM you what I would have done, but I don't actually delete anything or kick anyone.
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

Use /help anytime for commands and extra features (like dry mode).

My code [available on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).
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
    SetConfig {
        group_id: GroupId,
        compressed: String,
    },
    GetGroups,
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
            let mut config: Option<String> = None;
            for param in fragment.split('&') {
                if let Some(v) = param.strip_prefix("bot_id=") {
                    bot_id = v.parse().ok();
                } else if let Some(v) = param.strip_prefix("config=") {
                    config = Some(v.to_string());
                }
            }
            if let (Some(group_id), Some(compressed)) = (bot_id, config) {
                return ParsedDm::SetConfig {
                    group_id,
                    compressed,
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
        _ => ParsedDm::Unknown,
    }
}

fn render_group(group: &Group, editor_url: &str) -> String {
    format!("*{}*\n[View and Edit Settings]({})", group.name, editor_url)
}

impl BotDmApplication {
    /// The editor link for a group: its whole configuration, compressed into
    /// the hash.
    async fn editor_url(&self, user_id: UserId, group_id: GroupId) -> Result<String, Err> {
        let json = self
            .group_operator
            .get_config_json(user_id, group_id)
            .await?;
        Ok(format!(
            "{}#bot_id={}&config={}",
            self.webeditor_base_url.trim_end_matches('/'),
            group_id,
            lz_compress(&json)
        ))
    }
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
            ParsedDm::SetConfig {
                group_id,
                compressed,
            } => {
                let result = match lz_decompress(&compressed) {
                    Some(json) => {
                        self.group_operator
                            .set_config_json(user_id, group_id, &json)
                            .await
                    }
                    None => Err("Could not decompress the settings from the URL. \
                        Open the editor link again, make your changes, \
                        and send back the updated URL."
                        .into()),
                };
                match result {
                    Ok(()) => {
                        self.messenger
                            .send_dm(&user_id, "Group settings updated successfully.")
                            .await?;
                    }
                    Err(e) => {
                        self.messenger
                            .send_dm(&user_id, &format!("Failed to update group settings: {}", e))
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
                        let editor_url = self.editor_url(user_id, group.id).await?;
                        self.messenger
                            .send_dm(&user_id, &render_group(group, &editor_url))
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
        if !invitation.is_moderator_or_higher {
            self.messenger
                .send_dm(
                    &user_id,
                    "The invitation must have the moderator role or higher.",
                )
                .await?;
            return Ok(());
        }
        match self
            .group_operator
            .try_join_group(user_id, invitation)
            .await
        {
            Ok(group) => {
                self.messenger
                    .send_dm(&user_id, "Joined the group successfully!")
                    .await?;
                let editor_url = self.editor_url(user_id, group.id).await?;
                self.messenger
                    .send_dm(&user_id, &render_group(&group, &editor_url))
                    .await?;
            }
            Err(_) => {
                self.messenger
                    .send_dm(
                        &user_id,
                        "Failed to join the group. Check that the invitation is still valid and send it again.",
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

/// Renders the actions the moderator performed (or, in dry mode, would have
/// performed) as one sentence, e.g. "🛡 I moderated the message and kicked the
/// author".
///
/// The actions arrive already merged and in execution order, so the phrases are
/// joined in the order they are given. Dry mode only switches the tense.
fn describe_actions(actions: &[ModerationAction], dry_mode: bool) -> String {
    let phrases: Vec<String> = actions
        .iter()
        .map(|action| match (action, dry_mode) {
            (ModerationAction::ModerateMessage, false) => "moderated the message".to_owned(),
            (ModerationAction::ModerateMessage, true) => "moderate the message".to_owned(),
            // "set" reads the same in both tenses.
            (
                ModerationAction::SetAuthorObserver {
                    duration_minutes: 0,
                },
                _,
            ) => "set the author as observer".to_owned(),
            (
                ModerationAction::SetAuthorObserver {
                    duration_minutes: 1,
                },
                _,
            ) => "set the author as observer for 1 minute".to_owned(),
            (ModerationAction::SetAuthorObserver { duration_minutes }, _) => {
                format!("set the author as observer for {duration_minutes} minutes")
            }
            (
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
                false,
            ) => "kicked the author".to_owned(),
            (
                ModerationAction::KickAuthor {
                    delete_all_messages: false,
                },
                true,
            ) => "kick the author".to_owned(),
            (
                ModerationAction::KickAuthor {
                    delete_all_messages: true,
                },
                false,
            ) => "kicked the author and deleted all their messages".to_owned(),
            (
                ModerationAction::KickAuthor {
                    delete_all_messages: true,
                },
                true,
            ) => "kick the author and delete all their messages".to_owned(),
        })
        .collect();

    // A match always carries at least one action, but saying so plainly beats
    // claiming something that did not happen if one ever arrives empty.
    let joined = match phrases.split_last() {
        None if dry_mode => "take no action".to_owned(),
        None => "took no action".to_owned(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {}", rest.join(", "), last),
    };

    if dry_mode {
        format!("🛡 I would {joined}")
    } else {
        format!("🛡 I {joined}")
    }
}

#[async_trait]
impl ModerationNotificationReceiver for BotDmApplication {
    async fn send_moderation_notification(
        &self,
        user_id: UserId,
        group: &Group,
        actions: &[ModerationAction],
        performed: bool,
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
        let actions_text = describe_actions(actions, !performed);
        let reasons = reasons
            .iter()
            .map(|x| format!("• {}", x))
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!(
            "{} in *{}*!\n\n*The message:*\n{}\n\n*Reason:*\n{}\n\nIf it's a false positive, please [file an issue](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml)",
            actions_text, group.name, message, reasons,
        );
        self.messenger.send_dm(&user_id, &text).await
    }
}
