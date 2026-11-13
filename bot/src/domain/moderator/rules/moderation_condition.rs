//! What makes a rule match.
//!
//! Every leaf condition is a type of its own, in a module of its own under
//! `moderation_condition/`: its parameters, how they are normalized and checked
//! when an owner saves them ([`Condition`]), how it describes itself, and what
//! it is checked on ([`ConditionOn`]). This module lists them all once
//! (`conditions!`), which builds [`ModerationCondition`]: a condition as an
//! owner writes it — in the rules JSON, the editor and the database —, any
//! leaf in any place. A trigger takes only some of them: its module lists
//! those (`trigger_conditions!`), which builds the trigger's own tree type, the
//! one the bot checks, reached from a `ModerationCondition` by `TryFrom`,
//! which refuses a leaf the trigger does not take (US-C2).
//!
//! # Conditions are a tree, not a list
//! Besides the leaf conditions, there are three composites — `All`, `Any` and
//! `Not` — which carry other conditions, in every form above. A rule therefore
//! owns a *tree* of conditions. The rule list itself is already a disjunction
//! (any matching rule contributes its actions), so `Any` only adds
//! expressiveness when nested.
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
use serde::{Deserialize, Serialize};

pub(in crate::domain::moderator::rules) use context::ConditionContext;
pub use context::ConditionPorts;
pub(in crate::domain::moderator::rules) use needs::Needs;

mod context;
mod needs;
mod tree;

/// What every leaf condition is, whatever it is checked on: its settings, how
/// they are normalized and checked when an owner saves them, and what it asks
/// of the bot. Nothing outside a condition's own module knows what one checks
/// or how.
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

/// A leaf condition checked on `S`, the subject of a trigger: a
/// `GroupMessage` for a condition that reads the message, any
/// [`Member`](super::common::member::Member) for one that reads only the
/// member. A trigger lists only conditions that implement this for its
/// subject, so a condition in the wrong trigger does not compile.
#[async_trait]
pub(in crate::domain::moderator::rules) trait ConditionOn<S: ?Sized + Sync>:
    Condition
{
    /// The reason the subject matches, or `None` when it does not.
    async fn should_moderate(&self, ctx: &ConditionContext<'_, S>) -> Result<Option<String>, Err>;
}

/// One node of a trigger's condition tree, as the engine walks it: a
/// composite with its children, or a leaf checked on the trigger's subject.
pub(in crate::domain::moderator::rules) enum Node<'a, T: TriggerCondition> {
    All(&'a [T]),
    Any(&'a [T]),
    Not(&'a T),
    Leaf(&'a dyn ConditionOn<T::Subject>),
}

/// A trigger's condition tree — what `trigger_conditions!` builds for each
/// trigger —, as the engine evaluates it, whichever trigger it is.
pub(in crate::domain::moderator::rules) trait TriggerCondition:
    Clone + Eq + std::hash::Hash + Send + Sync + Sized
{
    /// What the trigger's conditions are checked on.
    type Subject: Sync;

    fn node(&self) -> Node<'_, Self>;
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

        /// A condition as an owner writes it: any leaf, in any place.
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
    MemberNameContainsWords => member_name_contains_words,
    MemberNameContainsLinksOutsideList => member_name_contains_links_outside_list,
    FlaggedByOmniModeration => flagged_by_omni_moderation,
    FlaggedByOpenRouterInstruction => flagged_by_open_router_instruction,
}

/// Builds a trigger's condition tree from the leaves it takes — one line
/// each, by the leaf's name — checked on the trigger's `$subject`:
/// `All`, `Any` and `Not` first, then the leaves. The tree converts from a
/// [`ModerationCondition`], refusing any leaf not listed (US-C2), and back.
macro_rules! trigger_conditions {
    ($name:ident on $subject:ty, $trigger:expr; $($variant:ident,)*) => {
        /// A condition tree of the trigger's own leaves only.
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            All { conditions: Vec<$name> },
            Any { conditions: Vec<$name> },
            Not { condition: Box<$name> },
            $($variant($crate::domain::moderator::rules::conditions::$variant),)*
        }

        impl $crate::domain::moderator::rules::moderation_condition::TriggerCondition for $name {
            type Subject = $subject;

            fn node(
                &self,
            ) -> $crate::domain::moderator::rules::moderation_condition::Node<'_, Self> {
                use $crate::domain::moderator::rules::moderation_condition::Node;
                match self {
                    Self::All { conditions } => Node::All(conditions),
                    Self::Any { conditions } => Node::Any(conditions),
                    Self::Not { condition } => Node::Not(condition),
                    $(Self::$variant(leaf) => Node::Leaf(leaf),)*
                }
            }
        }

        impl TryFrom<$crate::domain::moderator::rules::ModerationCondition> for $name {
            type Error = $crate::domain::moderator::ports::Err;

            fn try_from(
                condition: $crate::domain::moderator::rules::ModerationCondition,
            ) -> Result<Self, Self::Error> {
                use $crate::domain::moderator::rules::ModerationCondition as Any_;
                match condition {
                    Any_::All { conditions } => Ok(Self::All {
                        conditions: conditions
                            .into_iter()
                            .map(Self::try_from)
                            .collect::<Result<_, _>>()?,
                    }),
                    Any_::Any { conditions } => Ok(Self::Any {
                        conditions: conditions
                            .into_iter()
                            .map(Self::try_from)
                            .collect::<Result<_, _>>()?,
                    }),
                    Any_::Not { condition } => Ok(Self::Not {
                        condition: Box::new(Self::try_from(*condition)?),
                    }),
                    $(Any_::$variant(leaf) => Ok(Self::$variant(leaf)),)*
                    #[allow(unreachable_patterns)]
                    other => Err($crate::domain::moderator::rules::trigger::not_taken(
                        other.type_name(),
                        $trigger,
                    )),
                }
            }
        }

        impl From<$name> for $crate::domain::moderator::rules::ModerationCondition {
            fn from(condition: $name) -> Self {
                match condition {
                    $name::All { conditions } => Self::All {
                        conditions: conditions.into_iter().map(Self::from).collect(),
                    },
                    $name::Any { conditions } => Self::Any {
                        conditions: conditions.into_iter().map(Self::from).collect(),
                    },
                    $name::Not { condition } => Self::Not {
                        condition: Box::new(Self::from(*condition)),
                    },
                    $($name::$variant(leaf) => Self::$variant(leaf),)*
                }
            }
        }
    };
}
pub(in crate::domain::moderator::rules) use trigger_conditions;

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
