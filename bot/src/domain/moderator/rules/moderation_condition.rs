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
pub use context::ConditionPorts;
pub(in crate::domain::moderator::rules) use needs::Needs;

mod context;
mod needs;
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

    /// What the bot has to record about the messages it sees for this
    /// condition to answer. Most need nothing.
    fn needs(&self) -> Needs {
        Needs::default()
    }

    /// Whether the answer depends on what the *other* rules do with this
    /// message. Such a condition is answered after the others (see the
    /// pre-pass in `rules`), and may not sit under a `Not`.
    fn depends_on_other_rules(&self) -> bool {
        false
    }

    /// The API keys this condition carries, and what each is for, so they
    /// can be checked when an owner saves the rule.
    fn key_uses(&self) -> Vec<KeyUse> {
        Vec::new()
    }
}

/// What a key a condition carries is for: which provider to ask about it when
/// an owner saves the rule, and for what.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyUse {
    /// An OpenAI key, for the moderation endpoint.
    OpenAiModeration { api_key: String },
    /// An OpenRouter key, for a model.
    OpenRouterModel { api_key: String, model: String },
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

            /// The serde tag: the variant's name, as the rules JSON, the editor
            /// and the database spell it.
            pub fn type_name(&self) -> &'static str {
                match self {
                    Self::All { .. } => "All",
                    Self::Any { .. } => "Any",
                    Self::Not { .. } => "Not",
                    $(Self::$variant(_) => stringify!($variant),)*
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

    /// Everything the leaves of this tree need recorded, together.
    pub fn needs(&self) -> Needs {
        let mut needs = Needs::default();
        self.walk(&mut |node| {
            if let Some(leaf) = node.as_leaf() {
                needs = needs.merge(leaf.needs());
            }
        });
        needs
    }

    /// Whether a leaf anywhere in this tree depends on what the other rules
    /// do with the message. The evaluation pre-pass and the memo both have to
    /// treat such a subtree specially.
    pub fn depends_on_other_rules(&self) -> bool {
        let mut found = false;
        self.walk(&mut |node| {
            if node
                .as_leaf()
                .is_some_and(|leaf| leaf.depends_on_other_rules())
            {
                found = true;
            }
        });
        found
    }

    /// Every API key in this tree, with what it is for.
    pub fn key_uses(&self) -> Vec<KeyUse> {
        let mut uses = Vec::new();
        self.walk(&mut |node| {
            if let Some(leaf) = node.as_leaf() {
                uses.extend(leaf.key_uses());
            }
        });
        uses
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
        // A subtree that depends on the other rules evaluates differently
        // in the pre-pass, so its result is not safe to carry between the two
        // passes. Everything else is pure with respect to the pass and is
        // memoized once.
        let memoizable = !condition.depends_on_other_rules();
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
            // The pre-pass asks what the other rules do with the message, so
            // a condition that depends on them answers "no match" there
            // rather than about itself.
            Some(condition) if ctx.pre_pass && condition.depends_on_other_rules() => Ok(None),
            Some(condition) => condition.should_moderate(ctx).await,
            None => unreachable!("composites are matched above"),
        },
    }
}
