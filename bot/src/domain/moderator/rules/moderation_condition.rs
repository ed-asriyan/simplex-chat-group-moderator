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
//! - The tree is **bounded**: at most [`MAX_CONDITION_DEPTH`] levels and
//!   [`MAX_CONDITION_NODES`] nodes.

#[cfg(test)]
mod tests;

mod attachment;
mod character_rate_limit;
mod exact_message;
mod instruction_context;
mod invisible_chars;
mod joined_recently;
mod keywords;
mod line_rate_limit;
mod links;
mod message_length;
mod message_rate_limit;
mod moderation_rate_limit;
mod openai_moderation;
mod regex_match;
mod repeated_sequence;

use crate::domain::moderator::ports::Err;
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

pub(super) use context::ConditionContext;

mod context;

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
// Saving: normalization and limits
// ---------------------------------------------------------------------------

/// Maximum nesting depth of one rule's condition tree. A bare leaf has depth 1.
pub const MAX_CONDITION_DEPTH: usize = 8;

/// Maximum number of condition nodes (composites included) in one rule's tree.
pub const MAX_CONDITION_NODES: usize = 64;

/// Append `condition` to `out` unless an identical sibling is already there.
fn push_unique(out: &mut Vec<ModerationCondition>, condition: ModerationCondition) {
    if !out.contains(&condition) {
        out.push(condition);
    }
}

/// Normalize a composite's children and collapse the composite itself.
///
/// `is_all` selects which composite we are in, which decides both what counts
/// as a same-kind child worth flattening and what to rebuild at the end.
fn normalize_composite(
    children: Vec<ModerationCondition>,
    is_all: bool,
) -> Result<Option<ModerationCondition>, Err> {
    let mut out: Vec<ModerationCondition> = Vec::new();
    for child in children {
        let Some(child) = normalize_node(child)? else {
            continue;
        };
        let same_kind = match &child {
            ModerationCondition::All { .. } => is_all,
            ModerationCondition::Any { .. } => !is_all,
            _ => false,
        };
        if same_kind {
            let (ModerationCondition::All { conditions } | ModerationCondition::Any { conditions }) =
                child
            else {
                unreachable!("same_kind is only set for All/Any")
            };
            for nested in conditions {
                push_unique(&mut out, nested);
            }
        } else {
            push_unique(&mut out, child);
        }
    }
    match out.len() {
        // An empty composite carries no meaning. Vacuous truth would make an
        // empty `All` match every message, so it collapses to nothing instead
        // and the caller decides whether that is fatal.
        0 => Ok(None),
        1 => Ok(out.pop()),
        _ => Ok(Some(if is_all {
            ModerationCondition::All { conditions: out }
        } else {
            ModerationCondition::Any { conditions: out }
        })),
    }
}

/// Normalize one node bottom-up. `None` means the node collapsed to nothing.
///
/// Children are normalized before their parent, so a single pass reaches the
/// fixpoint: collapsing a child can make its parent collapsible, never the
/// other way round.
fn normalize_node(condition: ModerationCondition) -> Result<Option<ModerationCondition>, Err> {
    match condition {
        ModerationCondition::All { conditions } => normalize_composite(conditions, true),
        ModerationCondition::Any { conditions } => normalize_composite(conditions, false),
        ModerationCondition::Not { condition } => {
            let Some(inner) = normalize_node(*condition)? else {
                return Ok(None);
            };
            // Not(Not(a)) == a.
            if let ModerationCondition::Not { condition: inner } = inner {
                return Ok(Some(*inner));
            }
            Ok(Some(ModerationCondition::Not {
                condition: Box::new(inner),
            }))
        }
        mut leaf => {
            let Some(condition) = leaf.as_leaf_mut() else {
                unreachable!("composites are matched above")
            };
            condition.normalize_and_validate()?;
            Ok(Some(leaf))
        }
    }
}

fn depth_of(condition: &ModerationCondition) -> usize {
    match condition {
        ModerationCondition::All { conditions } | ModerationCondition::Any { conditions } => {
            1 + conditions.iter().map(depth_of).max().unwrap_or(0)
        }
        ModerationCondition::Not { condition } => 1 + depth_of(condition),
        _ => 1,
    }
}

/// Reject an `AuthorHitsModerationRateLimit` nested under a `Not`.
///
/// That condition asks "is this message moderated by some other rule", and the
/// pre-pass in `rules` answers it by evaluating every rule with these
/// nodes pinned to "no match". Under a negation, pinning a node to "no match"
/// can *cause* the enclosing rule to fire, so the answer would depend on itself.
/// The check is deliberately blind to negation parity: nothing useful is
/// expressed by a doubly negated one, so forbidding any `Not` ancestor
/// keeps both the rule and its explanation simple.
fn check_no_moderation_rate_limit_under_not(
    condition: &ModerationCondition,
    under_not: bool,
) -> Result<(), Err> {
    match condition {
        ModerationCondition::AuthorHitsModerationRateLimit(_) if under_not => Err(
            "'Author Hits Moderation Rate Limit' cannot be placed under a 'Not' \
             condition, because it already depends on what the other rules do with this message."
                .into(),
        ),
        ModerationCondition::All { conditions } | ModerationCondition::Any { conditions } => {
            for child in conditions {
                check_no_moderation_rate_limit_under_not(child, under_not)?;
            }
            Ok(())
        }
        ModerationCondition::Not { condition } => {
            check_no_moderation_rate_limit_under_not(condition, true)
        }
        _ => Ok(()),
    }
}

impl ModerationCondition {
    /// Bring the condition's parameters into canonical form and check them
    /// against the limits.
    ///
    /// Normalization and validation are one step on purpose: validating without
    /// normalizing would count blank and duplicate entries towards the limits,
    /// and normalizing without validating would let oversized values through.
    ///
    /// For a tree this also does the structural work described on the module:
    /// leaves first, then each composite is flattened, deduplicated and
    /// collapsed. A composite left with nothing in it disappears, which mirrors
    /// how blank list rows are dropped — the editor produces both from
    /// half-filled widgets rather than from any intent. If that leaves the whole
    /// rule without a condition the save is rejected, because silently dropping
    /// a rule the owner believes exists is worse than an error.
    pub fn normalize_and_validate(&mut self) -> Result<(), Err> {
        let placeholder = Self::All {
            conditions: Vec::new(),
        };
        let taken = std::mem::replace(self, placeholder);
        let Some(normalized) = normalize_node(taken)? else {
            return Err(
                "A rule has an empty condition. Add at least one condition to it, or remove the rule."
                    .into(),
            );
        };

        // Limits are checked after normalization: it only ever shrinks the tree,
        // so checking first would reject trees that are fine once canonical.
        let depth = depth_of(&normalized);
        if depth > MAX_CONDITION_DEPTH {
            return Err(format!(
                "Condition nesting is too deep: {depth} levels, maximum is {MAX_CONDITION_DEPTH}"
            )
            .into());
        }
        let mut nodes = 0usize;
        normalized.walk(&mut |_| nodes += 1);
        if nodes > MAX_CONDITION_NODES {
            return Err(format!(
                "Too many conditions in one rule: {nodes} provided, maximum is {MAX_CONDITION_NODES}"
            )
            .into());
        }
        check_no_moderation_rate_limit_under_not(&normalized, false)?;

        *self = normalized;
        Ok(())
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
