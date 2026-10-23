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

// Written in SimpleX's Markdown. Once a message holds any block (a heading, a
// list, a quote), a single newline inside a paragraph renders as a space, so
// every line meant to stand on its own is a block or is set off by a blank line.
// Commands stay bare: the app makes a bare `/command` tappable, a code span would
// not be.
const HELP: &str = "\
### How to use this bot

1. Invite me to your group as:
   - **Moderator** — to delete violating messages.
   - **Admin** — to also set violators as observers.
   - **Owner** — to also kick violators out of the group.
2. Once I join, send /groups to list your groups and open the visual editor.
3. I watch the chat and act according to your rules.

To stop me moderating a group, just remove me from it.

### Modes

Each group has a mode, set in the same editor as its rules (send /groups to get the link):

- **Notifications** — I act and DM you every time I do.
- **Silent** — I act without telling you.
- **Dry** — I run every check and DM you what I would have done, but delete and kick nothing.

### Commands

- /start — show the welcome guide.
- /help — show this guide.
- /groups — list and manage the groups I moderate for you.
- /source — view my source code.
- /issue — report a bug or unexpected moderation.
- /feature — request a new moderation rule type or feature.
";

const ISSUE_URL: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml";

const FEATURE_REQUEST_URL: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=feature-request.yml";

const SOURCE_URL: &str = "https://github.com/ed-asriyan/simplex-chat-group-moderator";

const START: &str = "\
Hi! I'm an **automated moderation** bot for SimpleX groups.

### What I can detect

- 🚫 **Banned words and exact phrases**, even obfuscated like b@d_w0rd.
- 🔗 **Links** — block specific sites, allow only a list, or allow just the Top 100 sites.
- ⏱ **Rate limits** — too many messages, or repeated violations, in a time window.
- 🌊 **Screen flooding** — long messages, empty or invisible text, line floods.

…and much more — this is just a glimpse.

### What I can do to violators

- 🗑 **Delete their messages**
- 👁 **Make them observers**
- 🚪 **Kick them out of the group**

### How to get started

1. Invite me to your group as:
   - **Moderator** — if you only want me to 🗑 delete messages.
   - **Admin** — if you also want me to 👁 make members observers.
   - **Owner** — if you also want me to 🚪 kick members.
2. Once I join, send /groups to set up your rules in the visual editor.

Send /help any time for the commands and modes (like dry mode).

My code is [available on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).
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

/// A group's name as a heading over the link to its editor. The heading is a
/// block of its own, so the link stays on its own line whatever the name holds.
fn render_group(group: &Group, editor_url: &str) -> String {
    format!(
        "### {}\n[View and edit settings]({})",
        group.name, editor_url
    )
}

/// `text` as a Markdown block quote, its line breaks kept: inside a quote a
/// plain newline renders as a space, so every line but the last ends in the two
/// spaces that make it a hard break. A quote's content is parsed inline only, so
/// nothing in `text` can open a block that runs past the quote.
fn quote(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if line.trim().is_empty() {
                ">".to_owned()
            } else if i + 1 < lines.len() {
                format!("> {line}  ")
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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
                            .send_dm(&user_id, "✅ Group settings updated.")
                            .await?;
                    }
                    Err(e) => {
                        let text = format!(
                            "❌ **Failed to update group settings:**\n\n{}",
                            quote(&e.to_string())
                        );
                        self.messenger.send_dm(&user_id, &text).await?;
                    }
                }
            }

            ParsedDm::GetGroups => {
                let groups = self.group_operator.get_groups(user_id).await?;
                if groups.is_empty() {
                    self.messenger
                        .send_dm(
                            &user_id,
                            "You don't have any groups yet. Invite me to your group as **moderator** \
                            or higher to get started — /help explains how.",
                        )
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
                let message = format!("My source code is [on GitHub]({SOURCE_URL}).");
                self.messenger.send_dm(&user_id, &message).await?;
            }
            ParsedDm::Issue => {
                let message = format!(
                    "🐞 Please report bugs and unexpected moderation [on GitHub]({}).",
                    ISSUE_URL
                );
                self.messenger.send_dm(&user_id, &message).await?;
            }
            ParsedDm::Feature => {
                let message = format!(
                    "💡 Please request new moderation rule types and features [on GitHub]({}).",
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
                    "❌ I can't join this group: I need the **moderator** role or higher to delete \
                    messages. Invite me again with that role.",
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
                    .send_dm(
                        &user_id,
                        "✅ I joined the group. Open the editor below to set up its rules.",
                    )
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
                        "❌ I couldn't join the group. Check that the invitation is still valid and send it again.",
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
        // Quoted, so the member's own text can neither pass for the bot's nor
        // break out of its place in the message.
        let message = if message.trim().is_empty() {
            "_(no text)_".to_owned()
        } else {
            quote(&message)
        };
        let reasons = reasons
            .iter()
            .map(|x| format!("- {}", x))
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!(
            "{} in **{}**.\n\n**The message:**\n{}\n\n**Reason:**\n{}\n\nIf it's a false positive, please [file an issue]({}).",
            actions_text, group.name, message, reasons, ISSUE_URL,
        );
        self.messenger.send_dm(&user_id, &text).await
    }
}
