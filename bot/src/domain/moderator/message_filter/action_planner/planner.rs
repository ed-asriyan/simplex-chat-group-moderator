use crate::domain::moderator::ports::{
    DeleteAuthorMessages, DeleteObserverMessages, ModerationAction,
};

/// An atomic/concrete action that the moderation engine can execute.
///
/// The variant order defines the safe execution order (top to bottom):
/// `SetObserver` -> `DeleteTriggeredMessage` -> `KickAuthor`. Sorting a normalized
/// set therefore yields the correct execution sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlannedAction {
    SetObserver,
    DeleteTriggeredMessage,
    KickAuthor { delete_all_messages: bool },
}

impl PlannedAction {
    /// Returns true if performing `self` already achieves everything `other` would,
    /// making `other` redundant. This is the single place that encodes how one
    /// concrete action subsumes another.
    fn covers(&self, other: &PlannedAction) -> bool {
        use PlannedAction::*;
        if self == other {
            return true;
        }
        match (self, other) {
            // Kicking with full message deletion covers a plain kick...
            (
                KickAuthor {
                    delete_all_messages: true,
                },
                KickAuthor {
                    delete_all_messages: false,
                },
            ) => true,
            // ...and covers deleting just the triggered message.
            (
                KickAuthor {
                    delete_all_messages: true,
                },
                DeleteTriggeredMessage,
            ) => true,
            // Kicking the author covers restricting them to observer (read-only).
            (KickAuthor { .. }, SetObserver) => true,
            _ => false,
        }
    }

    /// The concrete actions required to carry out a `ModerationAction`.
    fn for_moderation_action(action: &ModerationAction) -> Vec<PlannedAction> {
        match action {
            ModerationAction::ModerateMessage => vec![PlannedAction::DeleteTriggeredMessage],
            ModerationAction::SetAuthorObserver { delete_message } => {
                let mut actions = vec![PlannedAction::SetObserver];
                if *delete_message == DeleteObserverMessages::TriggeredMessage {
                    actions.push(PlannedAction::DeleteTriggeredMessage);
                }
                actions
            }
            ModerationAction::KickAuthor { delete_messages } => match delete_messages {
                DeleteAuthorMessages::None => vec![PlannedAction::KickAuthor {
                    delete_all_messages: false,
                }],
                DeleteAuthorMessages::TriggeredMessage => vec![
                    PlannedAction::DeleteTriggeredMessage,
                    PlannedAction::KickAuthor {
                        delete_all_messages: false,
                    },
                ],
                DeleteAuthorMessages::AllMessages => vec![PlannedAction::KickAuthor {
                    delete_all_messages: true,
                }],
            },
        }
    }
}

/// A normalized set of planned actions.
///
/// Invariant: never contains an action that is covered by another action in the set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PlannedActionSet {
    actions: Vec<PlannedAction>,
}

impl PlannedActionSet {
    fn from_actions(raw: &[PlannedAction]) -> Self {
        let mut set = Self::default();
        for &action in raw {
            set.add(action);
        }
        set
    }

    /// Adds an action with normalization:
    /// - if an existing action already covers it, it is dropped;
    /// - any existing actions it covers are removed.
    fn add(&mut self, action: PlannedAction) {
        if self.actions.iter().any(|existing| existing.covers(&action)) {
            return;
        }
        self.actions.retain(|existing| !action.covers(existing));
        self.actions.push(action);
    }

    /// Returns true if every action of `other` is covered by some action of `self`.
    fn covers(&self, other: &PlannedActionSet) -> bool {
        other
            .actions
            .iter()
            .all(|o| self.actions.iter().any(|s| s.covers(o)))
    }

    /// Merges two sets; any action covered by a stronger one disappears.
    fn union(&self, other: &PlannedActionSet) -> Self {
        let mut result = self.clone();
        for &action in &other.actions {
            result.add(action);
        }
        result
    }

    /// Actions in safe execution order (see `PlannedAction`'s variant order).
    fn into_ordered(self) -> Vec<PlannedAction> {
        let mut actions = self.actions;
        actions.sort();
        actions
    }

    /// Converts the normalized set into the canonical `ModerationAction` representation.
    fn to_moderation_action(&self) -> Option<ModerationAction> {
        let kick = self.actions.iter().find_map(|a| match a {
            PlannedAction::KickAuthor {
                delete_all_messages,
            } => Some(*delete_all_messages),
            _ => None,
        });
        let has_delete = self
            .actions
            .contains(&PlannedAction::DeleteTriggeredMessage);

        if let Some(delete_all_messages) = kick {
            let delete_messages = if delete_all_messages {
                DeleteAuthorMessages::AllMessages
            } else if has_delete {
                DeleteAuthorMessages::TriggeredMessage
            } else {
                DeleteAuthorMessages::None
            };
            Some(ModerationAction::KickAuthor { delete_messages })
        } else if self.actions.contains(&PlannedAction::SetObserver) {
            let delete_message = if has_delete {
                DeleteObserverMessages::TriggeredMessage
            } else {
                DeleteObserverMessages::None
            };
            Some(ModerationAction::SetAuthorObserver { delete_message })
        } else if has_delete {
            Some(ModerationAction::ModerateMessage)
        } else {
            None
        }
    }
}

/// Evaluates whether applying `rule_action` would add anything to `current_actions`.
///
/// - If `rule_action`'s actions are all covered by `current_actions`, the rule adds
///   nothing new, so it returns `None`. In the evaluation loop, this allows skipping
///   the rule's condition check completely.
/// - Otherwise the sets are merged (any action covered by a stronger one disappears)
///   and the resulting ordered actions are returned as `Some(new_actions)`.
pub fn plan_next_actions(
    current_actions: &[PlannedAction],
    rule_action: &ModerationAction,
) -> Option<Vec<PlannedAction>> {
    let current = PlannedActionSet::from_actions(current_actions);
    let candidate =
        PlannedActionSet::from_actions(&PlannedAction::for_moderation_action(rule_action));

    if current.covers(&candidate) {
        None
    } else {
        Some(current.union(&candidate).into_ordered())
    }
}

/// Converts a non-empty list of planned actions into the corresponding consolidated `ModerationAction`.
pub fn planned_actions_to_moderation_action(actions: &[PlannedAction]) -> ModerationAction {
    PlannedActionSet::from_actions(actions)
        .to_moderation_action()
        .unwrap_or(ModerationAction::ModerateMessage)
}
