//! Types the moderator context exchanges across its ports.

pub use crate::domain::moderator::rules::{
    ModerationAction, ModerationCondition, ModerationMatch, ModerationRule, actions, conditions,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

#[cfg(test)]
mod tests;

pub type MessengerGroupId = i64;
pub type GroupId = i64;
pub type MessageId = i64;
pub type UserId = i64;

pub type Err = Box<dyn Error + Send + Sync>;

#[derive(Clone, Debug)]
pub struct Group {
    pub id: GroupId,
    pub owner_id: UserId,
    pub name: String,
    pub mode: GroupMode,
}

/// How the bot treats a match in a group: whether it carries out the rule's
/// actions, and whether it tells the owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupMode {
    /// Checks every message and tells the owner what it would have done, but
    /// does nothing in the group.
    Dry,
    /// Carries out the actions without telling the owner.
    Silent,
    /// Carries out the actions and tells the owner about each one.
    #[default]
    Notifications,
}

impl GroupMode {
    pub fn executes_actions(self) -> bool {
        self != Self::Dry
    }

    pub fn notifies_owner(self) -> bool {
        self != Self::Silent
    }

    /// The name used in the group's JSON and in the database.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dry => "dry",
            Self::Silent => "silent",
            Self::Notifications => "notifications",
        }
    }

    /// The mode whose [`Self::name`] is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Dry, Self::Silent, Self::Notifications]
            .into_iter()
            .find(|mode| mode.name() == name)
    }
}

/// Everything an owner configures about a group, saved and loaded as one: the
/// mode and the rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupConfig {
    pub mode: GroupMode,
    pub rules: Vec<ModerationRule>,
}

#[derive(Clone, Debug, Default)]
pub struct MessengerGroup {
    pub id: MessengerGroupId,
    pub name: String,
}

/// What a message carries besides its text. The messenger sends one attachment
/// per message, with the caption as the message text, so this is one kind and
/// not a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAttachment {
    Image,
    Video,
    Voice,
    File,
}

#[derive(Clone, Debug, Default)]
pub struct GroupMessage {
    pub group: MessengerGroup,
    pub message_id: MessageId,
    pub author_id: UserId,
    /// The name the author chose for themselves, as other members see it. Not
    /// unique and not stable: it identifies nobody, `author_id` does.
    pub author_name: String,
    pub text: String,
    /// What the message carries besides its text; `None` for a plain text
    /// message. An attachment's caption is the `text` above, so a picture with
    /// no caption is a message with an attachment and blank text.
    pub attachment: Option<MessageAttachment>,
    pub timestamp: DateTime<Utc>,
    /// When the author joined the group, if known. Only members who joined
    /// after the bot have a known join time; for everyone who was already there
    /// when the bot joined this is `None`.
    pub author_joined_at: Option<DateTime<Utc>>,
    /// An edit of a message already posted, rather than a new message. It is
    /// moderated like any other message — an edit into a banned word is still
    /// caught — but it is not new traffic, so it feeds no rate limit counter.
    /// `timestamp` is then when the edit was made.
    pub is_edit: bool,
}

#[derive(Clone, Debug)]
pub struct OwnedModerationRule {
    pub id: usize,
    pub rule: ModerationRule,
}

/// The roles the bot moves a member between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMemberRole {
    Member,
    Observer,
}

/// A member the bot still owes a restore to: it made them an observer for a
/// limited time, and at `execute_at` that time is up.
#[derive(Clone, Debug)]
pub struct ScheduledMemberRestore {
    pub id: i64,
    pub messenger_group_id: MessengerGroupId,
    pub member_id: UserId,
    pub execute_at: DateTime<Utc>,
}

/// What the provider — OpenAI or OpenRouter — said about an API key an owner
/// is saving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCheck {
    /// The key can make the request the condition will make.
    Valid,
    /// The provider does not know the key (401): mistyped, revoked or
    /// disabled.
    Rejected,
    /// The key exists but may not make that request (403): an OpenAI key
    /// without the Moderations permission, or an OpenRouter key whose
    /// guardrail does not allow the model.
    Forbidden,
    /// The account behind the key cannot pay (OpenAI: 429
    /// `insufficient_quota`; OpenRouter: 402, or the key's own credit limit is
    /// spent). Retrying does not help.
    QuotaExceeded,
    /// The model cannot be asked this way (400/404): a model the provider
    /// retired, or one that has no endpoint left taking the request.
    ModelUnavailable,
    /// The provider could not be asked: network trouble, 5xx, rate limiting,
    /// or an answer that could not be read. Says nothing about the key itself.
    Unreachable,
}

/// A model's answer about one message: whether it is what the owner's
/// instruction describes, and the model's own one-sentence reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRouterInstructionVerdict {
    pub matches: bool,
    pub reason: String,
}

/// One of a group's latest messages, kept for an OpenRouter instruction to
/// read as context. `timestamp` is when it was posted, or last edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentGroupMessage {
    pub message_id: MessageId,
    pub author_id: UserId,
    pub author_name: String,
    pub text: String,
    pub attachment: Option<MessageAttachment>,
    pub timestamp: DateTime<Utc>,
}

/// An earlier message as it is sent to a model: read for context, not judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionContextMessage {
    pub author_name: String,
    pub text: String,
    pub attachment: Option<MessageAttachment>,
}

/// How hard a condition tries its provider — OpenAI or OpenRouter — before
/// reading a failure as "no verdict": `max_attempts` calls in all (1 means no
/// retry), `retry_delay_seconds` apart. Only a failure that can pass — the
/// provider's trouble, a timeout, rate limiting — is tried again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiRetry {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_retry_delay_seconds")]
    pub retry_delay_seconds: u32,
}

fn default_max_attempts() -> u32 {
    3
}

fn default_retry_delay_seconds() -> u32 {
    1
}

impl Default for ApiRetry {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            retry_delay_seconds: default_retry_delay_seconds(),
        }
    }
}

impl ApiRetry {
    /// One call, never repeated.
    pub const NONE: Self = Self {
        max_attempts: 1,
        retry_delay_seconds: 0,
    };
}

/// One of the categories `omni-moderation-latest` scores a text in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiCategory {
    Sexual,
    SexualMinors,
    Harassment,
    HarassmentThreatening,
    Hate,
    HateThreatening,
    Illicit,
    IllicitViolent,
    SelfHarm,
    SelfHarmIntent,
    SelfHarmInstructions,
    Violence,
    ViolenceGraphic,
}

impl OpenAiCategory {
    /// Every category, in the order reasons list them.
    pub const ALL: [Self; 13] = [
        Self::Sexual,
        Self::SexualMinors,
        Self::Harassment,
        Self::HarassmentThreatening,
        Self::Hate,
        Self::HateThreatening,
        Self::Illicit,
        Self::IllicitViolent,
        Self::SelfHarm,
        Self::SelfHarmIntent,
        Self::SelfHarmInstructions,
        Self::Violence,
        Self::ViolenceGraphic,
    ];

    /// The name OpenAI's API uses for this category, e.g. `hate/threatening`.
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Sexual => "sexual",
            Self::SexualMinors => "sexual/minors",
            Self::Harassment => "harassment",
            Self::HarassmentThreatening => "harassment/threatening",
            Self::Hate => "hate",
            Self::HateThreatening => "hate/threatening",
            Self::Illicit => "illicit",
            Self::IllicitViolent => "illicit/violent",
            Self::SelfHarm => "self-harm",
            Self::SelfHarmIntent => "self-harm/intent",
            Self::SelfHarmInstructions => "self-harm/instructions",
            Self::Violence => "violence",
            Self::ViolenceGraphic => "violence/graphic",
        }
    }

    /// The name used in the rules JSON and in the database, e.g.
    /// `hate_threatening`: the API's names carry slashes and dashes, which make
    /// poor field names.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sexual => "sexual",
            Self::SexualMinors => "sexual_minors",
            Self::Harassment => "harassment",
            Self::HarassmentThreatening => "harassment_threatening",
            Self::Hate => "hate",
            Self::HateThreatening => "hate_threatening",
            Self::Illicit => "illicit",
            Self::IllicitViolent => "illicit_violent",
            Self::SelfHarm => "self_harm",
            Self::SelfHarmIntent => "self_harm_intent",
            Self::SelfHarmInstructions => "self_harm_instructions",
            Self::Violence => "violence",
            Self::ViolenceGraphic => "violence_graphic",
        }
    }

    /// The category whose [`Self::name`] is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.name() == name)
    }

    /// The category whose [`Self::api_name`] is `api_name`, if any. Categories
    /// OpenAI adds later are unknown here and come back as `None`.
    pub fn from_api_name(api_name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.api_name() == api_name)
    }
}

/// What makes one category match.
///
/// On the wire (the rules JSON) it is `"off"`, `"openai"` or an integer
/// percentage, which is what the editor's select-or-number control produces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "TriggerWire", into = "TriggerWire")]
pub enum CategoryTrigger {
    /// The category is ignored. The default for a field missing from the JSON:
    /// deleting a message the owner did not ask to delete is worse than
    /// missing one.
    #[default]
    Off,
    /// Matches when OpenAI itself flags the category.
    OpenAiDecides,
    /// Matches when OpenAI's score for the category, as a percentage, is at
    /// least this. Valid values are 1..=100; others are rejected on save.
    MinScorePercent(u8),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TriggerWire {
    Word(String),
    Percent(u8),
}

impl TryFrom<TriggerWire> for CategoryTrigger {
    type Error = String;

    fn try_from(wire: TriggerWire) -> Result<Self, Self::Error> {
        match wire {
            TriggerWire::Word(word) => match word.as_str() {
                "off" => Ok(Self::Off),
                "openai" => Ok(Self::OpenAiDecides),
                other => Err(format!(
                    "unknown category trigger '{other}', expected \"off\", \"openai\" or a percentage"
                )),
            },
            TriggerWire::Percent(percent) => Ok(Self::MinScorePercent(percent)),
        }
    }
}

impl From<CategoryTrigger> for TriggerWire {
    fn from(trigger: CategoryTrigger) -> Self {
        match trigger {
            CategoryTrigger::Off => Self::Word("off".to_string()),
            CategoryTrigger::OpenAiDecides => Self::Word("openai".to_string()),
            CategoryTrigger::MinScorePercent(percent) => Self::Percent(percent),
        }
    }
}

/// The owner's trigger for every category. Field names are
/// [`OpenAiCategory::name`]; a field missing from the JSON is `Off`.
///
/// A setting of `FlaggedByOmniModeration` rather than something a port
/// carries: it lives here because the repository adapter reads and writes it,
/// and a condition's own module exports nothing but the condition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpenAiCategoryTriggers {
    #[serde(default)]
    pub sexual: CategoryTrigger,
    #[serde(default)]
    pub sexual_minors: CategoryTrigger,
    #[serde(default)]
    pub harassment: CategoryTrigger,
    #[serde(default)]
    pub harassment_threatening: CategoryTrigger,
    #[serde(default)]
    pub hate: CategoryTrigger,
    #[serde(default)]
    pub hate_threatening: CategoryTrigger,
    #[serde(default)]
    pub illicit: CategoryTrigger,
    #[serde(default)]
    pub illicit_violent: CategoryTrigger,
    #[serde(default)]
    pub self_harm: CategoryTrigger,
    #[serde(default)]
    pub self_harm_intent: CategoryTrigger,
    #[serde(default)]
    pub self_harm_instructions: CategoryTrigger,
    #[serde(default)]
    pub violence: CategoryTrigger,
    #[serde(default)]
    pub violence_graphic: CategoryTrigger,
}

impl OpenAiCategoryTriggers {
    /// Every category set to `trigger`.
    pub fn all(trigger: CategoryTrigger) -> Self {
        let mut triggers = Self::default();
        for category in OpenAiCategory::ALL {
            triggers.set(category, trigger);
        }
        triggers
    }

    pub fn get(&self, category: OpenAiCategory) -> CategoryTrigger {
        *self.field(category)
    }

    pub fn set(&mut self, category: OpenAiCategory, trigger: CategoryTrigger) {
        *self.field_mut(category) = trigger;
    }

    fn field(&self, category: OpenAiCategory) -> &CategoryTrigger {
        match category {
            OpenAiCategory::Sexual => &self.sexual,
            OpenAiCategory::SexualMinors => &self.sexual_minors,
            OpenAiCategory::Harassment => &self.harassment,
            OpenAiCategory::HarassmentThreatening => &self.harassment_threatening,
            OpenAiCategory::Hate => &self.hate,
            OpenAiCategory::HateThreatening => &self.hate_threatening,
            OpenAiCategory::Illicit => &self.illicit,
            OpenAiCategory::IllicitViolent => &self.illicit_violent,
            OpenAiCategory::SelfHarm => &self.self_harm,
            OpenAiCategory::SelfHarmIntent => &self.self_harm_intent,
            OpenAiCategory::SelfHarmInstructions => &self.self_harm_instructions,
            OpenAiCategory::Violence => &self.violence,
            OpenAiCategory::ViolenceGraphic => &self.violence_graphic,
        }
    }

    fn field_mut(&mut self, category: OpenAiCategory) -> &mut CategoryTrigger {
        match category {
            OpenAiCategory::Sexual => &mut self.sexual,
            OpenAiCategory::SexualMinors => &mut self.sexual_minors,
            OpenAiCategory::Harassment => &mut self.harassment,
            OpenAiCategory::HarassmentThreatening => &mut self.harassment_threatening,
            OpenAiCategory::Hate => &mut self.hate,
            OpenAiCategory::HateThreatening => &mut self.hate_threatening,
            OpenAiCategory::Illicit => &mut self.illicit,
            OpenAiCategory::IllicitViolent => &mut self.illicit_violent,
            OpenAiCategory::SelfHarm => &mut self.self_harm,
            OpenAiCategory::SelfHarmIntent => &mut self.self_harm_intent,
            OpenAiCategory::SelfHarmInstructions => &mut self.self_harm_instructions,
            OpenAiCategory::Violence => &mut self.violence,
            OpenAiCategory::ViolenceGraphic => &mut self.violence_graphic,
        }
    }
}

/// OpenAI's verdict on one text, in this context's own terms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenAiModerationResult {
    /// The categories OpenAI flagged by its own thresholds.
    pub flagged: BTreeSet<OpenAiCategory>,
    /// OpenAI's score per category, 0.0..=1.0. A category missing here scores 0.
    pub scores: BTreeMap<OpenAiCategory, f64>,
}
