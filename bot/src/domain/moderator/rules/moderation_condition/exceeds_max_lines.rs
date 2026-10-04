//! `ExceedsMaxLines`: the message takes more lines on screen than the owner
//! allows.

#[cfg(test)]
mod tests;

use super::{Condition, ConditionContext};
use crate::domain::moderator::ports::Err;
use crate::domain::moderator::rules::common::checks;
use crate::domain::moderator::rules::common::screen_lines::count_effective_lines;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The message takes more than `max_lines` lines, with every line longer
/// than `chars_per_line` characters counted as several (0: no wrapping).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExceedsMaxLines {
    pub max_lines: u32,
    pub chars_per_line: u32,
}

#[async_trait]
impl Condition for ExceedsMaxLines {
    fn normalize_and_validate(&mut self) -> Result<(), Err> {
        checks::check_nonzero(
            self.max_lines,
            "'Message Exceeds Max Lines' needs a maximum of at least 1 line",
        )
    }

    fn describe(&self) -> String {
        format!("has more than {} lines", self.max_lines)
    }

    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err> {
        Ok(should_moderate_lines(
            &ctx.group_message.text,
            self.max_lines,
            self.chars_per_line,
        ))
    }
}

/// Matches a message that takes more than `max_lines` lines on screen. A line
/// longer than `chars_per_line` characters counts as several lines, as it
/// would when soft-wrapped on a narrow screen; 0 disables the wrapping.
fn should_moderate_lines(message: &str, max_lines: u32, chars_per_line: u32) -> Option<String> {
    // Validation rejects 0 on save; a stored 0 still never matches, so a bad
    // value is inert rather than matching every message.
    if max_lines == 0 {
        return None;
    }
    let count = count_effective_lines(message, chars_per_line);
    (count > max_lines as usize).then(|| format!("{count} lines"))
}
