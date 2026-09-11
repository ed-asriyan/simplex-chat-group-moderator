//! What makes a message match a rule.
//!
//! `ModerationCondition` is the single source of truth for detection types: the
//! PascalCase variant name is the serde tag used in the rules URL and
//! `rules-schema.json`, and its snake_case form names the condition's database
//! table. Everything a condition does lives here — its parameters, how they are
//! normalized and checked when an owner saves them, and how one is evaluated
//! against a message. The per-condition matching algorithms themselves live in
//! the sibling modules (`keywords`, `links`, `message_length`, ...).
//!
//! # Conditions are a tree, not a list
//! Besides the leaf conditions that inspect a message, there are three composite
//! variants — [`ModerationCondition::All`], [`ModerationCondition::Any`] and
//! [`ModerationCondition::Not`] — which carry other conditions. A rule therefore
//! owns a *tree* of conditions whose root is the rule's `condition` field. The
//! rule list itself is already a disjunction (any matching rule contributes its
//! action), so `Any` only adds expressiveness when nested.
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

mod exact_message;
mod invisible_chars;
mod joined_recently;
mod keywords;
mod links;
mod message_length;
mod message_rate_limit;
mod moderation_rate_limit;
mod regex_match;
mod repeated_sequence;

use crate::domain::moderator::ports::Err;
use futures::future::BoxFuture;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub(super) use context::ConditionContext;

mod context;

fn deserialize_u32_default_zero<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<u32>::deserialize(deserializer)?;
    Ok(opt.unwrap_or(0))
}

/// Condition/filter criteria for moderation.
///
/// Names describe what the message *is*, never what the owner thinks of it
/// ("contains words", not "contains banned words"): the same condition can sit
/// under a `Not`, where a judgement baked into the name would read backwards.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModerationCondition {
    /// Matches when **every** nested condition matches.
    All {
        conditions: Vec<ModerationCondition>,
    },
    /// Matches when **at least one** nested condition matches.
    Any {
        conditions: Vec<ModerationCondition>,
    },
    /// Matches when the nested condition does **not** match.
    Not {
        condition: Box<ModerationCondition>,
    },
    /// The message contains any of `keywords`, seen through obfuscation.
    ContainsWords {
        keywords: Vec<String>,
    },
    /// The whole message equals one of `messages`.
    MatchesExactMessage {
        messages: Vec<String>,
        case_sensitive: bool,
    },
    MatchesRegex {
        patterns: Vec<String>,
    },
    /// Some sequence of at least `min_length` characters appears at least
    /// `min_repeats` times in a row.
    ContainsRepeatedSequence {
        min_repeats: u32,
        min_length: u32,
    },
    /// The message links to a domain covered by `domains`.
    ContainsLinksInList {
        domains: Vec<String>,
    },
    /// The message links to a domain not covered by `domains`. An empty list
    /// makes every link match.
    ContainsLinksOutsideList {
        domains: Vec<String>,
    },
    /// The message links to a domain covered neither by the built-in top-100
    /// list nor by `domains`.
    ContainsLinksOutsideTop100 {
        domains: Vec<String>,
    },
    /// The message consists only of whitespace, line breaks and invisible
    /// characters, or has no characters at all.
    IsBlank,
    /// The message contains at least one invisible character.
    ContainsInvisibleCharacters,
    ExceedsMaxCharacters {
        max_characters: u32,
    },
    ExceedsMaxWords {
        max_words: u32,
    },
    /// The message takes more than `max_lines` lines, with every line longer
    /// than `chars_per_line` characters counted as several (0: no wrapping).
    ExceedsMaxLines {
        max_lines: u32,
        chars_per_line: u32,
    },
    /// The author sent at least `message_count` messages in the last
    /// `time_window_minutes`, this one included. 0 in either disables it.
    AuthorHitsMessageRateLimit {
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        message_count: u32,
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        time_window_minutes: u32,
    },
    /// At least `message_count` of the author's messages were moderated in the
    /// last `time_window_minutes`, this one included if another rule moderates
    /// it. 0 in either disables it.
    AuthorHitsModerationRateLimit {
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        message_count: u32,
        #[serde(default, deserialize_with = "deserialize_u32_default_zero")]
        time_window_minutes: u32,
    },
    /// The author joined the group less than `time_window_minutes` ago. Never
    /// matches members who were already in the group when the bot joined,
    /// since their join time is unknown.
    AuthorJoinedRecently {
        time_window_minutes: u32,
    },
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

    /// Whether a [`Self::AuthorHitsModerationRateLimit`] sits anywhere in this
    /// tree. That condition is the one whose result depends on whether the
    /// message is moderated by *other* rules, so both the evaluation pre-pass
    /// and the memo have to treat any subtree containing it specially.
    pub fn contains_moderation_rate_limit(&self) -> bool {
        let mut found = false;
        self.walk(&mut |condition| {
            if matches!(condition, Self::AuthorHitsModerationRateLimit { .. }) {
                found = true;
            }
        });
        found
    }

    /// Longest non-zero `time_window_minutes` over every
    /// [`Self::AuthorHitsMessageRateLimit`] in this tree.
    pub fn max_message_rate_limit_window(&self) -> Option<u32> {
        let mut max: Option<u32> = None;
        self.walk(&mut |condition| {
            if let Self::AuthorHitsMessageRateLimit {
                time_window_minutes,
                ..
            } = condition
                && *time_window_minutes > 0
            {
                max = Some(max.map_or(*time_window_minutes, |m: u32| m.max(*time_window_minutes)));
            }
        });
        max
    }

    /// Longest non-zero `time_window_minutes` over every
    /// [`Self::AuthorHitsModerationRateLimit`] in this tree.
    pub fn max_moderation_rate_limit_window(&self) -> Option<u32> {
        let mut max: Option<u32> = None;
        self.walk(&mut |condition| {
            if let Self::AuthorHitsModerationRateLimit {
                time_window_minutes,
                ..
            } = condition
                && *time_window_minutes > 0
            {
                max = Some(max.map_or(*time_window_minutes, |m: u32| m.max(*time_window_minutes)));
            }
        });
        max
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
            Self::ContainsWords { .. } => "contains one of the listed words".into(),
            Self::MatchesExactMessage { .. } => "exactly matches one of the listed texts".into(),
            Self::MatchesRegex { .. } => "matches a regex pattern".into(),
            Self::ContainsRepeatedSequence { .. } => "contains a repeated sequence".into(),
            Self::ContainsLinksInList { .. } => "contains a link to a listed website".into(),
            Self::ContainsLinksOutsideList { .. } => {
                "contains a link to a website outside the list".into()
            }
            Self::ContainsLinksOutsideTop100 { .. } => {
                "contains a link outside the top 100 websites".into()
            }
            Self::IsBlank => "is empty or blank".into(),
            Self::ContainsInvisibleCharacters => "contains invisible characters".into(),
            Self::ExceedsMaxCharacters { max_characters } => {
                format!("has more than {max_characters} characters")
            }
            Self::ExceedsMaxWords { max_words } => format!("has more than {max_words} words"),
            Self::ExceedsMaxLines { max_lines, .. } => format!("has more than {max_lines} lines"),
            Self::AuthorHitsMessageRateLimit {
                message_count,
                time_window_minutes,
            } => format!(
                "author sent at least {message_count} messages in {time_window_minutes} min"
            ),
            Self::AuthorHitsModerationRateLimit {
                message_count,
                time_window_minutes,
            } => format!(
                "author had at least {message_count} messages moderated in {time_window_minutes} min"
            ),
            Self::AuthorJoinedRecently {
                time_window_minutes,
            } => format!("author joined less than {time_window_minutes} min ago"),
        }
    }
}

// ---------------------------------------------------------------------------
// Saving: normalization and limits
// ---------------------------------------------------------------------------

/// Maximum number of entries in a single condition's keyword / message / domain list.
const MAX_LIST_ENTRIES: usize = 10_000;

/// Maximum length (in characters) of a single keyword or domain.
const MAX_KEYWORD_LENGTH: usize = 100;

/// Maximum length (in characters) of a single exact message.
const MAX_MESSAGE_LENGTH: usize = 1000;

/// Maximum number of regex patterns in a single condition. Far lower than the other
/// lists because compiled patterns are memoized in a fixed-size process-wide cache
/// (see `regex_match`): this bounds how much of that shared cache one rule can claim,
/// keeping the cache useful for every other group.
const MAX_REGEX_PATTERNS: usize = 100;

/// Maximum length (in characters) of a single regex pattern.
const MAX_REGEX_PATTERN_LENGTH: usize = 200;

/// Maximum nesting depth of one rule's condition tree. A bare leaf has depth 1.
pub const MAX_CONDITION_DEPTH: usize = 8;

/// Maximum number of condition nodes (composites included) in one rule's tree.
pub const MAX_CONDITION_NODES: usize = 64;

/// Drop blank entries and reduce the list to a sorted set. Blank entries are
/// dropped rather than rejected because the editor's list widget produces them
/// whenever a row is added and left untouched.
fn normalize_list(values: &mut Vec<String>) {
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
}

/// `noun` names the list in the "too many" message, `entry` a single entry in
/// the "too long" one, so both read naturally for keywords, messages and domains.
fn check_list(values: &[String], max_length: usize, noun: &str, entry: &str) -> Result<(), Err> {
    if values.len() > MAX_LIST_ENTRIES {
        return Err(format!(
            "Too many {noun}: {} provided, maximum is {MAX_LIST_ENTRIES}",
            values.len()
        )
        .into());
    }
    if let Some(value) = values.iter().find(|v| v.chars().count() > max_length) {
        return Err(format!(
            "{entry} too long: {} characters, maximum is {max_length}",
            value.chars().count()
        )
        .into());
    }
    Ok(())
}

/// Reject a zero where the condition could otherwise never match. The error
/// names the condition as the editor titles it, so the owner can find it.
fn check_nonzero(value: u32, error: &str) -> Result<(), Err> {
    if value == 0 {
        return Err(error.into());
    }
    Ok(())
}

/// Bring a leaf condition's parameters into canonical form and check them.
fn normalize_and_validate_leaf(condition: &mut ModerationCondition) -> Result<(), Err> {
    match condition {
        ModerationCondition::ContainsWords { keywords } => {
            normalize_list(keywords);
            check_list(keywords, MAX_KEYWORD_LENGTH, "keywords", "Keyword")
        }
        ModerationCondition::MatchesExactMessage { messages, .. } => {
            normalize_list(messages);
            check_list(messages, MAX_MESSAGE_LENGTH, "messages", "Message")
        }
        ModerationCondition::MatchesRegex { patterns } => {
            normalize_list(patterns);
            if patterns.len() > MAX_REGEX_PATTERNS {
                return Err(format!(
                    "Too many regex patterns: {} provided, maximum is {MAX_REGEX_PATTERNS}",
                    patterns.len()
                )
                .into());
            }
            if let Some(pattern) = patterns
                .iter()
                .find(|p| p.chars().count() > MAX_REGEX_PATTERN_LENGTH)
            {
                return Err(format!(
                    "Regex pattern too long: {} characters, maximum is {MAX_REGEX_PATTERN_LENGTH}",
                    pattern.chars().count()
                )
                .into());
            }
            // Rejecting here is what lets the matcher treat a non-compiling
            // pattern as "never matches" instead of surfacing errors per message.
            if let Some(pattern) = patterns.iter().find(|p| Regex::new(p).is_err()) {
                return Err(format!("Invalid regex pattern: '{pattern}'").into());
            }
            Ok(())
        }
        ModerationCondition::ContainsRepeatedSequence {
            min_repeats,
            min_length,
        } => {
            // A single occurrence is not a repetition: 1 (or 0) would match
            // every non-empty message.
            if *min_repeats < 2 {
                return Err(
                    format!("Minimum repeats must be at least 2, got {min_repeats}").into(),
                );
            }
            if !(1..=repeated_sequence::MAX_SEQUENCE_LENGTH).contains(min_length) {
                return Err(format!(
                    "Minimum sequence length must be between 1 and {}, got {min_length}",
                    repeated_sequence::MAX_SEQUENCE_LENGTH
                )
                .into());
            }
            Ok(())
        }
        ModerationCondition::ContainsLinksInList { domains }
        | ModerationCondition::ContainsLinksOutsideList { domains }
        | ModerationCondition::ContainsLinksOutsideTop100 { domains } => {
            normalize_list(domains);
            check_list(domains, MAX_KEYWORD_LENGTH, "domains", "Domain")
        }
        // Unlike `AuthorHitsMessageRateLimit`, where 0 means "disabled", a zero
        // here would store a rule that can never match while the owner believes
        // it protects the group.
        ModerationCondition::ExceedsMaxCharacters { max_characters } => check_nonzero(
            *max_characters,
            "'Message Exceeds Max Characters' needs a maximum of at least 1 character",
        ),
        ModerationCondition::ExceedsMaxWords { max_words } => check_nonzero(
            *max_words,
            "'Message Exceeds Max Words' needs a maximum of at least 1 word",
        ),
        ModerationCondition::ExceedsMaxLines { max_lines, .. } => check_nonzero(
            *max_lines,
            "'Message Exceeds Max Lines' needs a maximum of at least 1 line",
        ),
        ModerationCondition::AuthorJoinedRecently {
            time_window_minutes,
        } => check_nonzero(
            *time_window_minutes,
            "'Author Joined Recently' needs a time window of at least 1 minute",
        ),
        ModerationCondition::IsBlank
        | ModerationCondition::ContainsInvisibleCharacters
        | ModerationCondition::AuthorHitsMessageRateLimit { .. }
        | ModerationCondition::AuthorHitsModerationRateLimit { .. } => Ok(()),
        ModerationCondition::All { .. }
        | ModerationCondition::Any { .. }
        | ModerationCondition::Not { .. } => {
            unreachable!("composites are normalized by normalize_node")
        }
    }
}

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
            normalize_and_validate_leaf(&mut leaf)?;
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
/// pre-pass in `message_filter` answers it by evaluating every rule with these
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
        ModerationCondition::AuthorHitsModerationRateLimit { .. } if under_not => Err(
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

fn should_moderate_by_condition(message: &str, condition: &ModerationCondition) -> Option<String> {
    match condition {
        ModerationCondition::ContainsWords { keywords } => {
            keywords::should_moderate(message.trim(), keywords)
                .map(|keyword| format!("contains word: '{keyword}'"))
        }
        ModerationCondition::MatchesExactMessage {
            messages,
            case_sensitive,
        } => exact_message::should_moderate(message.trim(), messages, *case_sensitive),
        ModerationCondition::MatchesRegex { patterns } => {
            regex_match::should_moderate(message, patterns)
                .map(|pattern| format!("matches regex pattern: '{pattern}'"))
        }
        ModerationCondition::ContainsRepeatedSequence {
            min_repeats,
            min_length,
        } => repeated_sequence::should_moderate(message, *min_repeats, *min_length),
        ModerationCondition::ContainsLinksInList { domains } => {
            links::should_moderate_in_list(message.trim(), domains)
        }
        ModerationCondition::ContainsLinksOutsideList { domains } => {
            links::should_moderate_outside_list(message.trim(), domains)
        }
        ModerationCondition::ContainsLinksOutsideTop100 { domains } => {
            links::should_moderate_outside_top100(message.trim(), domains)
        }
        // The shape conditions see the raw message: leading and trailing
        // whitespace is exactly what they are measuring.
        ModerationCondition::IsBlank => invisible_chars::should_moderate_blank(message),
        ModerationCondition::ContainsInvisibleCharacters => {
            invisible_chars::should_moderate_invisible(message)
        }
        ModerationCondition::ExceedsMaxCharacters { max_characters } => {
            message_length::should_moderate_characters(message, *max_characters)
        }
        ModerationCondition::ExceedsMaxWords { max_words } => {
            message_length::should_moderate_words(message, *max_words)
        }
        ModerationCondition::ExceedsMaxLines {
            max_lines,
            chars_per_line,
        } => message_length::should_moderate_lines(message, *max_lines, *chars_per_line),
        // Repository-backed, author-based and composite conditions never reach
        // here; they are handled by `evaluate` because they need the context.
        ModerationCondition::AuthorHitsMessageRateLimit { .. }
        | ModerationCondition::AuthorHitsModerationRateLimit { .. }
        | ModerationCondition::AuthorJoinedRecently { .. }
        | ModerationCondition::All { .. }
        | ModerationCondition::Any { .. }
        | ModerationCondition::Not { .. } => None,
    }
}

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
        ModerationCondition::AuthorHitsMessageRateLimit {
            message_count,
            time_window_minutes,
        } => {
            message_rate_limit::check(
                ctx.activity_repo,
                &ctx.group_message.group.id,
                &ctx.group_message.author_id,
                *message_count,
                *time_window_minutes,
                ctx.group_message.timestamp,
            )
            .await
        }
        ModerationCondition::AuthorHitsModerationRateLimit { .. }
            if ctx.moderation_rate_limit_pinned =>
        {
            Ok(None)
        }
        ModerationCondition::AuthorHitsModerationRateLimit {
            message_count,
            time_window_minutes,
        } if ctx.message_is_moderated => {
            if *message_count == 0 || *time_window_minutes == 0 {
                Ok(None)
            } else {
                let since = ctx.group_message.timestamp
                    - chrono::Duration::minutes(*time_window_minutes as i64);
                let count = ctx
                    .moderation_activity_repo
                    .count_moderated_messages_since(
                        &ctx.group_message.group.id,
                        &ctx.group_message.author_id,
                        since,
                        ctx.group_message.timestamp,
                    )
                    .await?
                    + 1;
                Ok(moderation_rate_limit::should_moderate(
                    count,
                    *message_count,
                    *time_window_minutes,
                ))
            }
        }
        ModerationCondition::AuthorHitsModerationRateLimit {
            message_count,
            time_window_minutes,
        } => {
            moderation_rate_limit::check(
                ctx.moderation_activity_repo,
                &ctx.group_message.group.id,
                &ctx.group_message.author_id,
                *message_count,
                *time_window_minutes,
                ctx.group_message.timestamp,
            )
            .await
        }
        ModerationCondition::AuthorJoinedRecently {
            time_window_minutes,
        } => Ok(joined_recently::should_moderate(
            ctx.group_message.author_joined_at,
            ctx.group_message.timestamp,
            *time_window_minutes,
        )),
        other => Ok(should_moderate_by_condition(&ctx.group_message.text, other)),
    }
}
