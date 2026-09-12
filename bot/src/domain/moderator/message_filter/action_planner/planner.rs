use crate::domain::moderator::ports::ModerationAction;

/// Where an action sits in the safe execution order: restricting the author to
/// observer, then moderating the triggering message, then kicking the author.
///
/// Kicking last is what makes the order safe — once the author is out of the
/// group, acting on them or on their message may no longer be possible. The
/// order deliberately does *not* come from the order the owner listed the
/// actions in (or from `ModerationAction`'s variant order, which is the
/// owner-facing one): a rule's action list is a set, and this module is the only
/// place that decides how a set of actions is carried out.
fn execution_rank(action: &ModerationAction) -> u8 {
    match action {
        ModerationAction::SetAuthorObserver { .. } => 0,
        ModerationAction::ModerateMessage => 1,
        ModerationAction::KickAuthor { .. } => 2,
    }
}

/// Returns true if performing `action` already achieves everything `other`
/// would, making `other` redundant. This is the single place that encodes how
/// one action subsumes another.
fn covers(action: &ModerationAction, other: &ModerationAction) -> bool {
    use ModerationAction::*;
    if action == other {
        return true;
    }
    match (action, other) {
        // Kicking with full message deletion covers a plain kick...
        (
            KickAuthor {
                delete_all_messages: true,
            },
            KickAuthor {
                delete_all_messages: false,
            },
        ) => true,
        // ...and covers moderating just the triggering message.
        (
            KickAuthor {
                delete_all_messages: true,
            },
            ModerateMessage,
        ) => true,
        // Kicking the author covers restricting them to observer (read-only).
        (KickAuthor { .. }, SetAuthorObserver { .. }) => true,
        // Between two observer restrictions the stricter one wins: holding the
        // author indefinitely (0) covers every timed restriction, and a longer
        // timer covers a shorter one.
        (
            SetAuthorObserver {
                duration_minutes: held,
            },
            SetAuthorObserver {
                duration_minutes: other_held,
            },
        ) => *held == 0 || (*other_held != 0 && held >= other_held),
        _ => false,
    }
}

/// A normalized set of actions.
///
/// Invariant: never contains an action that is covered by another action in the set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ActionSet {
    actions: Vec<ModerationAction>,
}

impl ActionSet {
    fn from_actions(raw: &[ModerationAction]) -> Self {
        let mut set = Self::default();
        for &action in raw {
            set.add(action);
        }
        set
    }

    /// Adds an action with normalization:
    /// - if an existing action already covers it, it is dropped;
    /// - any existing actions it covers are removed.
    fn add(&mut self, action: ModerationAction) {
        if self
            .actions
            .iter()
            .any(|existing| covers(existing, &action))
        {
            return;
        }
        self.actions.retain(|existing| !covers(&action, existing));
        self.actions.push(action);
    }

    /// Returns true if every action of `other` is covered by some action of `self`.
    fn covers_set(&self, other: &ActionSet) -> bool {
        other
            .actions
            .iter()
            .all(|o| self.actions.iter().any(|s| covers(s, o)))
    }

    /// Merges two sets; any action covered by a stronger one disappears.
    fn union(&self, other: &ActionSet) -> Self {
        let mut result = self.clone();
        for &action in &other.actions {
            result.add(action);
        }
        result
    }

    /// Actions in safe execution order (see [`execution_rank`]).
    fn into_ordered(self) -> Vec<ModerationAction> {
        let mut actions = self.actions;
        actions.sort_by_key(execution_rank);
        actions
    }
}

/// Normalizes a rule's action list into the canonical form: duplicates and
/// actions covered by a stronger one dropped, the rest in execution order.
///
/// Used both when an owner saves a rule (so what is stored is what will run) and
/// on every merge below.
pub fn normalize_actions(actions: &[ModerationAction]) -> Vec<ModerationAction> {
    ActionSet::from_actions(actions).into_ordered()
}

/// Evaluates whether applying `rule_actions` would add anything to `current_actions`.
///
/// - If every action of `rule_actions` is covered by `current_actions`, the rule
///   adds nothing new, so it returns `None`. In the evaluation loop, this allows
///   skipping the rule's condition check completely.
/// - Otherwise the sets are merged (any action covered by a stronger one disappears)
///   and the resulting ordered actions are returned as `Some(new_actions)`.
pub fn plan_next_actions(
    current_actions: &[ModerationAction],
    rule_actions: &[ModerationAction],
) -> Option<Vec<ModerationAction>> {
    let current = ActionSet::from_actions(current_actions);
    let candidate = ActionSet::from_actions(rule_actions);

    if current.covers_set(&candidate) {
        None
    } else {
        Some(current.union(&candidate).into_ordered())
    }
}
