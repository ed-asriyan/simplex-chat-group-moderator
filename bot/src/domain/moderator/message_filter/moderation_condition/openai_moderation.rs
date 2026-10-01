//! `FlaggedByOmniModeration`: reading OpenAI's moderation verdict on a
//! message against the owner's per-category triggers.
//!
//! OpenAI answers with two things per category: its own yes/no (`categories`,
//! decided by thresholds only OpenAI knows) and a score between 0 and 1
//! (`category_scores`). The owner picks, per category, which of the two to
//! trust — or neither. A single `flagged` switch on top of numeric thresholds
//! would be worse: OpenAI's own yes/no would fire first and make any threshold
//! more lenient than OpenAI's unreachable.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

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

/// Most tries an owner may ask for: each one can wait out the full request
/// timeout, and the message waits with it. Mirrored by `max_attempts` in
/// `rules-schema.json`.
pub const MAX_OPENAI_ATTEMPTS: u32 = 5;

/// Longest pause an owner may ask for between two tries, in seconds. Mirrored
/// by `retry_delay_seconds` in `rules-schema.json`.
pub const MAX_OPENAI_RETRY_DELAY_SECONDS: u32 = 10;

/// How hard a condition tries OpenAI before reading a failure as "no verdict":
/// `max_attempts` calls in all (1 means no retry), `retry_delay_seconds` apart.
/// Only a failure that can pass — OpenAI's trouble, a timeout, rate limiting —
/// is tried again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpenAiRetry {
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

impl Default for OpenAiRetry {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            retry_delay_seconds: default_retry_delay_seconds(),
        }
    }
}

impl OpenAiRetry {
    /// One call, never repeated.
    pub const NONE: Self = Self {
        max_attempts: 1,
        retry_delay_seconds: 0,
    };

    /// Checked when an owner saves the rule.
    pub fn validate(&self, title: &str) -> Result<(), String> {
        if !(1..=MAX_OPENAI_ATTEMPTS).contains(&self.max_attempts) {
            return Err(format!(
                "'{title}' needs between 1 and {MAX_OPENAI_ATTEMPTS} attempts, got {}",
                self.max_attempts
            ));
        }
        if self.retry_delay_seconds > MAX_OPENAI_RETRY_DELAY_SECONDS {
            return Err(format!(
                "'{title}' can wait at most {MAX_OPENAI_RETRY_DELAY_SECONDS} seconds between attempts, got {}",
                self.retry_delay_seconds
            ));
        }
        Ok(())
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

/// The reason `verdict` trips `triggers`, or `None` when no category does.
///
/// Categories are ORed: one is enough. The reason lists every category that
/// tripped, in [`OpenAiCategory::ALL`] order, e.g.
/// `flagged by OpenAI Omni: hate (OpenAI), violence 91% ≥ 80%`.
pub fn should_moderate(
    triggers: &OpenAiCategoryTriggers,
    verdict: &OpenAiModerationResult,
) -> Option<String> {
    let tripped: Vec<String> = OpenAiCategory::ALL
        .into_iter()
        .filter_map(|category| match triggers.get(category) {
            CategoryTrigger::Off => None,
            CategoryTrigger::OpenAiDecides => verdict
                .flagged
                .contains(&category)
                .then(|| format!("{} (OpenAI)", category.api_name())),
            CategoryTrigger::MinScorePercent(min) => {
                let percent = verdict.scores.get(&category).copied().unwrap_or(0.0) * 100.0;
                reaches(percent, min)
                    .then(|| format!("{} {percent:.0}% ≥ {min}%", category.api_name()))
            }
        })
        .collect();
    if tripped.is_empty() {
        None
    } else {
        Some(format!("flagged by OpenAI Omni: {}", tripped.join(", ")))
    }
}

/// Whether a score, in percent, reaches the owner's whole-number threshold.
///
/// Scores arrive as fractions, and turning one into a percentage is not
/// exact: 0.29 * 100.0 is 28.999999999999996. The tolerance is far below any
/// difference OpenAI's scores can express, so it only ever absorbs that error.
fn reaches(percent: f64, min_percent: u8) -> bool {
    const TOLERANCE: f64 = 1e-9;
    percent + TOLERANCE >= f64::from(min_percent)
}
