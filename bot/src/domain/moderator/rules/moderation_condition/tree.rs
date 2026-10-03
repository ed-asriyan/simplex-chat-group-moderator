//! The tree the conditions form: how a rule's tree is brought into canonical
//! form and kept within bounds when an owner saves it. Only the composites are
//! known here; a leaf is asked to normalize itself.

use super::ModerationCondition;
use crate::domain::moderator::ports::Err;

#[cfg(test)]
mod tests;

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
