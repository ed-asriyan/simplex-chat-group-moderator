//! What makes a message match a rule.
//!
//! `ModerationCondition` is the single source of truth for detection types: the
//! PascalCase variant name is the serde tag used in the rules URL and
//! `rules-schema.json`, and its snake_case form names the condition's database
//! table. Each leaf condition is a type of its own, in a module of its own,
//! implementing [`Condition`]: its parameters, how they are normalized and
//! checked when an owner saves them, how it describes itself, and how it is
//! evaluated against a message. This module only lists them (`conditions!`)
//! and handles what no single condition can: the tree they form.
//!
//! # Conditions are a tree, not a list
//! Besides the leaf conditions that inspect a message, there are three composite
//! variants — [`ModerationCondition::All`], [`ModerationCondition::Any`] and
//! [`ModerationCondition::Not`] — which carry other conditions. A rule therefore
//! owns a *tree* of conditions whose root is the rule's `condition` field. The
//! rule list itself is already a disjunction (any matching rule contributes its
//! actions), so `Any` only adds expressiveness when nested.
//!
//! Two invariants keep that tree manageable, both established by
//! [`ModerationCondition::normalize_and_validate`] before anything is stored:
//! - The tree is **canonical**: no empty composites, no composite nested
//!   directly in a composite of the same kind, no duplicate siblings, no
//!   single-child composites, no double negation.
//! - The tree is **bounded**: at most [`tree::MAX_CONDITION_DEPTH`] levels and
//!   [`tree::MAX_CONDITION_NODES`] nodes.

#[cfg(test)]
mod tests;

use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

pub(super) use context::ConditionContext;

mod context;
mod tree;

/// What every leaf condition is to the code that evaluates a rule: a set of
/// operations, the same for all of them. Nothing outside a condition's own
/// module knows what one checks or how.
#[async_trait]
pub(in crate::domain::moderator::rules) trait Condition:
    Send + Sync
{
    /// Brings the condition's parameters into canonical form and checks them,
    /// when an owner saves the rule. One step on purpose: validating without
    /// normalizing would count blank and duplicate entries towards the
    /// limits, and normalizing without validating would let oversized values
    /// through.
    fn normalize_and_validate(&mut self) -> Result<(), Err>;

    /// What the condition looks for, distinct from the reason evaluation
    /// gives for a match. A `Not` around it needs this: when the `Not`
    /// matches, the condition did not, so it produced no reason of its own.
    fn describe(&self) -> String;

    /// The reason the message matches, or `None` when it does not.
    async fn should_moderate(&self, ctx: &mut ConditionContext<'_>) -> Result<Option<String>, Err>;
}

/// Declares every leaf condition — one line each, `Variant => module` — and
/// builds [`ModerationCondition`] from them, with the three composites first.
macro_rules! conditions {
    ($($variant:ident => $module:ident,)*) => {
        $(mod $module;)*

        /// Every leaf condition's own type, for code that builds or reads one.
        pub mod conditions {
            $(pub use super::$module::$variant;)*
        }

        /// Condition/filter criteria for moderation.
        ///
        /// Names describe what the message *is*, never what the owner thinks of
        /// it ("contains words", not "contains banned words"): the same
        /// condition can sit under a `Not`, where a judgement baked into the
        /// name would read backwards.
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(tag = "type")]
        pub enum ModerationCondition {
            /// Matches when **every** nested condition matches.
            All { conditions: Vec<ModerationCondition> },
            /// Matches when **at least one** nested condition matches.
            Any { conditions: Vec<ModerationCondition> },
            /// Matches when the nested condition does **not** match.
            Not { condition: Box<ModerationCondition> },
            $($variant($module::$variant),)*
        }

        impl ModerationCondition {
            /// The leaf condition this is, or `None` for a composite.
            fn as_leaf(&self) -> Option<&dyn Condition> {
                match self {
                    $(Self::$variant(leaf) => Some(leaf),)*
                    Self::All { .. } | Self::Any { .. } | Self::Not { .. } => None,
                }
            }

            fn as_leaf_mut(&mut self) -> Option<&mut dyn Condition> {
                match self {
                    $(Self::$variant(leaf) => Some(leaf),)*
                    Self::All { .. } | Self::Any { .. } | Self::Not { .. } => None,
                }
            }
        }
    };
}

conditions! {
    ContainsWords => contains_words,
    MatchesExactMessage => matches_exact_message,
    MatchesRegex => matches_regex,
    ContainsRepeatedSequence => contains_repeated_sequence,
    ContainsLinksInList => contains_links_in_list,
    ContainsLinksOutsideList => contains_links_outside_list,
    ContainsLinksOutsideTop100 => contains_links_outside_top100,
    IsBlank => is_blank,
    ContainsInvisibleCharacters => contains_invisible_characters,
    ContainsImage => contains_image,
    ContainsVideo => contains_video,
    ContainsVoiceMessage => contains_voice_message,
    ContainsFile => contains_file,
    ExceedsMaxCharacters => exceeds_max_characters,
    ExceedsMaxWords => exceeds_max_words,
    ExceedsMaxLines => exceeds_max_lines,
    AuthorHitsMessageRateLimit => author_hits_message_rate_limit,
    AuthorHitsCharacterRateLimit => author_hits_character_rate_limit,
    AuthorHitsLineRateLimit => author_hits_line_rate_limit,
    AuthorHitsModerationRateLimit => author_hits_moderation_rate_limit,
    GroupHitsMessageRateLimit => group_hits_message_rate_limit,
    GroupHitsCharacterRateLimit => group_hits_character_rate_limit,
    GroupHitsLineRateLimit => group_hits_line_rate_limit,
    AuthorJoinedRecently => author_joined_recently,
    FlaggedByOmniModeration => flagged_by_omni_moderation,
    FlaggedByOpenRouterInstruction => flagged_by_open_router_instruction,
}

// ---------------------------------------------------------------------------
// Tree navigation
// ---------------------------------------------------------------------------

/// Longest non-zero window over every node `pick` reads one from.
fn max_window(
    condition: &ModerationCondition,
    pick: impl Fn(&ModerationCondition) -> Option<u32>,
) -> Option<u32> {
    let mut max: Option<u32> = None;
    condition.walk(&mut |node| {
        if let Some(window) = pick(node).filter(|window| *window > 0) {
            max = Some(max.map_or(window, |m| m.max(window)));
        }
    });
    max
}

/// The widest wrap width over every node `pick` reads one from, where 0 ("no
/// wrapping") is widest of all. `pick` gives `None` for a node without a
/// width, or with no window to count in.
fn widest_wrap_width(
    condition: &ModerationCondition,
    pick: impl Fn(&ModerationCondition) -> Option<u32>,
) -> Option<u32> {
    let mut width: Option<u32> = None;
    condition.walk(&mut |node| {
        if let Some(configured) = pick(node) {
            width = Some(match (width, configured) {
                // 0 means "no wrapping", which is wider than any width.
                (_, 0) | (Some(0), _) => 0,
                (Some(current), configured) => current.max(configured),
                (None, configured) => configured,
            });
        }
    });
    width
}

impl ModerationCondition {
    /// Visit this condition and, depth-first, every condition nested in it.
    ///
    /// Anything that needs to reason about "does this rule use X" must go
    /// through here: a condition can sit at any depth inside the composites, so
    /// matching on the rule's root condition alone silently misses nested ones.
    pub fn walk(&self, visit: &mut impl FnMut(&Self)) {
        visit(self);
        match self {
            Self::All { conditions } | Self::Any { conditions } => {
                for condition in conditions {
                    condition.walk(visit);
                }
            }
            Self::Not { condition } => condition.walk(visit),
            _ => {}
        }
    }

    /// Whether a `AuthorHitsModerationRateLimit` sits anywhere in this
    /// tree. That condition is the one whose result depends on whether the
    /// message is moderated by *other* rules, so both the evaluation pre-pass
    /// and the memo have to treat any subtree containing it specially.
    pub fn contains_moderation_rate_limit(&self) -> bool {
        let mut found = false;
        self.walk(&mut |condition| {
            if matches!(condition, Self::AuthorHitsModerationRateLimit(_)) {
                found = true;
            }
        });
        found
    }

    /// Longest non-zero `time_window_minutes` over every
    /// `AuthorHitsMessageRateLimit` in this tree.
    pub fn max_message_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::AuthorHitsMessageRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Longest non-zero `time_window_minutes` over every
    /// `AuthorHitsCharacterRateLimit` in this tree.
    pub fn max_character_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::AuthorHitsCharacterRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Longest non-zero `time_window_minutes` over every
    /// `AuthorHitsLineRateLimit` in this tree.
    pub fn max_line_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::AuthorHitsLineRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// The wrap width every `AuthorHitsLineRateLimit` in this tree is
    /// counted with: the widest one configured, where 0 ("no wrapping") is
    /// widest of all.
    ///
    /// One counter serves the whole group, so a message has one line count even
    /// when two conditions disagree about the width. Taking the widest makes
    /// that count the one the mildest condition expects: deleting a message the
    /// owner did not ask to delete is worse than missing a flood.
    pub fn line_rate_limit_wrap_width(&self) -> Option<u32> {
        widest_wrap_width(self, |node| match node {
            Self::AuthorHitsLineRateLimit(c) if c.time_window_minutes > 0 => Some(c.chars_per_line),
            _ => None,
        })
    }

    /// Longest non-zero `time_window_minutes` over every
    /// `AuthorHitsModerationRateLimit` in this tree.
    pub fn max_moderation_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::AuthorHitsModerationRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Longest `time_window_minutes` over every
    /// `GroupHitsMessageRateLimit` in this tree.
    pub fn max_group_message_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::GroupHitsMessageRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Longest `time_window_minutes` over every
    /// `GroupHitsCharacterRateLimit` in this tree.
    pub fn max_group_character_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::GroupHitsCharacterRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Longest `time_window_minutes` over every
    /// `GroupHitsLineRateLimit` in this tree.
    pub fn max_group_line_rate_limit_window(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::GroupHitsLineRateLimit(c) => Some(c.time_window_minutes),
            _ => None,
        })
    }

    /// Most `context_messages` over every
    /// `FlaggedByOpenRouterInstruction` in this tree: how many of the
    /// group's latest messages have to be kept for it to read. `None` when no
    /// condition here asks for any.
    pub fn max_openrouter_context_messages(&self) -> Option<u32> {
        max_window(self, |node| match node {
            Self::FlaggedByOpenRouterInstruction(c) => Some(c.context_messages),
            _ => None,
        })
    }

    /// The wrap width every `GroupHitsLineRateLimit` in this tree is
    /// counted with, chosen the way [`Self::line_rate_limit_wrap_width`] chooses
    /// it for the author's counter: the widest, with 0 widest of all. The group
    /// counter is separate, so the two widths never mix.
    pub fn group_line_rate_limit_wrap_width(&self) -> Option<u32> {
        widest_wrap_width(self, |node| match node {
            Self::GroupHitsLineRateLimit(c) if c.time_window_minutes > 0 => Some(c.chars_per_line),
            _ => None,
        })
    }

    /// Human-readable description of *what this condition looks for*.
    ///
    /// Distinct from the reason string produced by evaluation, which says what
    /// a message actually tripped over. [`Self::Not`] needs this: when it
    /// matches, its child did not, so there is no child reason to report.
    pub fn describe(&self) -> String {
        match self {
            Self::All { conditions } => format!(
                "all of ({})",
                conditions
                    .iter()
                    .map(Self::describe)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Self::Any { conditions } => format!(
                "any of ({})",
                conditions
                    .iter()
                    .map(Self::describe)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Self::Not { condition } => format!("not ({})", condition.describe()),
            leaf => match leaf.as_leaf() {
                Some(condition) => condition.describe(),
                None => unreachable!("composites are matched above"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Evaluate one condition, reusing the per-message memo.
///
/// Returns the reason the message matched, or `None` when it did not.
///
/// Boxed because the composites make this mutually recursive with `evaluate`,
/// and an `async fn` that (indirectly) awaits itself has no finite size.
pub(super) fn check_condition<'a>(
    ctx: &'a mut ConditionContext<'_>,
    condition: &'a ModerationCondition,
) -> BoxFuture<'a, Result<Option<String>, Err>> {
    Box::pin(async move {
        if let Some(cached) = ctx.memo.get(condition) {
            return Ok(cached.clone());
        }
        // A subtree containing `AuthorHitsModerationRateLimit` evaluates
        // differently depending on whether those nodes are pinned, so
        // its result is not safe to carry between the two passes. Everything
        // else is pure with respect to the pass and is memoized once.
        let memoizable = !condition.contains_moderation_rate_limit();
        let result = evaluate(ctx, condition).await?;
        if memoizable {
            ctx.memo.insert(condition.clone(), result.clone());
        }
        Ok(result)
    })
}
async fn evaluate(
    ctx: &mut ConditionContext<'_>,
    condition: &ModerationCondition,
) -> Result<Option<String>, Err> {
    match condition {
        ModerationCondition::All { conditions } => {
            // Normalization removes empty composites; treat one as "no match"
            // anyway rather than letting vacuous truth moderate every message.
            if conditions.is_empty() {
                return Ok(None);
            }
            let mut reasons = Vec::with_capacity(conditions.len());
            for child in conditions {
                // Short-circuit: condition evaluation has no side effects
                // (activity is recorded before the rules run), so skipping the
                // remaining children is unobservable apart from the saved work.
                let Some(reason) = check_condition(ctx, child).await? else {
                    return Ok(None);
                };
                reasons.push(reason);
            }
            Ok(Some(reasons.join(" and ")))
        }
        ModerationCondition::Any { conditions } => {
            for child in conditions {
                if let Some(reason) = check_condition(ctx, child).await? {
                    return Ok(Some(reason));
                }
            }
            Ok(None)
        }
        ModerationCondition::Not { condition: inner } => {
            if check_condition(ctx, inner).await?.is_some() {
                Ok(None)
            } else {
                // The child did not match, so it produced no reason of its own;
                // describe what was expected instead.
                Ok(Some(format!("does not match: {}", inner.describe())))
            }
        }
        leaf => match leaf.as_leaf() {
            Some(condition) => condition.should_moderate(ctx).await,
            None => unreachable!("composites are matched above"),
        },
    }
}
