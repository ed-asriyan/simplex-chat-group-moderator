//! `FlaggedByOmniModeration`: reading OpenAI's moderation verdict on a
//! message against the owner's per-category triggers.
//!
//! OpenAI answers with two things per category: its own yes/no (`categories`,
//! decided by thresholds only OpenAI knows) and a score between 0 and 1
//! (`category_scores`). The owner picks, per category, which of the two to
//! trust — or neither. A single `flagged` switch on top of numeric thresholds
//! would be worse: OpenAI's own yes/no would fire first and make any threshold
//! more lenient than OpenAI's unreachable.

use crate::domain::moderator::ports::{
    CategoryTrigger, OpenAiCategory, OpenAiCategoryTriggers, OpenAiModerationResult,
};

#[cfg(test)]
mod tests;

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
