//! `FlaggedByOpenAiModeration`: reading OpenAI's moderation verdict on a
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
        todo!("OpenAiCategory::api_name")
    }

    /// The name used in the rules JSON and in the database, e.g.
    /// `hate_threatening`: the API's names carry slashes and dashes, which make
    /// poor field names.
    pub fn name(self) -> &'static str {
        todo!("OpenAiCategory::name")
    }

    /// The category whose [`Self::name`] is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        let _ = name;
        todo!("OpenAiCategory::from_name")
    }

    /// The category whose [`Self::api_name`] is `api_name`, if any. Categories
    /// OpenAI adds later are unknown here and come back as `None`.
    pub fn from_api_name(api_name: &str) -> Option<Self> {
        let _ = api_name;
        todo!("OpenAiCategory::from_api_name")
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
        let _ = trigger;
        todo!("OpenAiCategoryTriggers::all")
    }

    pub fn get(&self, category: OpenAiCategory) -> CategoryTrigger {
        let _ = category;
        todo!("OpenAiCategoryTriggers::get")
    }

    pub fn set(&mut self, category: OpenAiCategory, trigger: CategoryTrigger) {
        let _ = (category, trigger);
        todo!("OpenAiCategoryTriggers::set")
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
/// `flagged by OpenAI moderation: hate (OpenAI), violence 91% ≥ 80%`.
#[allow(dead_code)] // red: evaluation does not call it yet
pub fn should_moderate(
    triggers: &OpenAiCategoryTriggers,
    verdict: &OpenAiModerationResult,
) -> Option<String> {
    let _ = (triggers, verdict);
    todo!("openai_moderation::should_moderate")
}
