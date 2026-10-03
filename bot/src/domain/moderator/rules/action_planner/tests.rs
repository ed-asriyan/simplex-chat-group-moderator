use super::planner::*;
use crate::domain::moderator::ports::ModerationAction;

const MODERATE: ModerationAction = ModerationAction::ModerateMessage;
const OBSERVER: ModerationAction = ModerationAction::SetAuthorObserver {
    duration_minutes: 0,
};
const KICK: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: false,
};
const KICK_ALL: ModerationAction = ModerationAction::KickAuthor {
    delete_all_messages: true,
};

#[test]
fn test_normalize_single_action_is_unchanged() {
    assert_eq!(normalize_actions(&[MODERATE]), vec![MODERATE]);
    assert_eq!(normalize_actions(&[OBSERVER]), vec![OBSERVER]);
    assert_eq!(normalize_actions(&[KICK_ALL]), vec![KICK_ALL]);
}

#[test]
fn test_normalize_drops_duplicates() {
    assert_eq!(normalize_actions(&[MODERATE, MODERATE]), vec![MODERATE]);
}

#[test]
fn test_normalize_orders_actions_for_safe_execution() {
    // Whatever order the owner listed them in, the author is kicked last...
    assert_eq!(normalize_actions(&[KICK, MODERATE]), vec![MODERATE, KICK]);
    // ...and restricted to observer first.
    assert_eq!(
        normalize_actions(&[MODERATE, OBSERVER]),
        vec![OBSERVER, MODERATE]
    );
}

#[test]
fn test_normalize_drops_actions_a_stronger_one_covers() {
    // Deleting every message of the author's already moderates this one, and
    // kicking them already keeps them from writing.
    assert_eq!(
        normalize_actions(&[MODERATE, OBSERVER, KICK_ALL]),
        vec![KICK_ALL]
    );
    // A plain kick does not delete the message, so moderation survives it.
    assert_eq!(normalize_actions(&[MODERATE, KICK]), vec![MODERATE, KICK]);
    // ...but it does cover the observer role.
    assert_eq!(normalize_actions(&[OBSERVER, KICK]), vec![KICK]);
    // The stronger kick wins over the weaker one.
    assert_eq!(normalize_actions(&[KICK, KICK_ALL]), vec![KICK_ALL]);
}

#[test]
fn test_plan_from_empty_plan_takes_the_rule_actions() {
    assert_eq!(plan_next_actions(&[], &[MODERATE]), Some(vec![MODERATE]));
    assert_eq!(
        plan_next_actions(&[], &[OBSERVER, MODERATE]),
        Some(vec![OBSERVER, MODERATE])
    );
    assert_eq!(
        plan_next_actions(&[], &[MODERATE, KICK]),
        Some(vec![MODERATE, KICK])
    );
}

#[test]
fn test_plan_normalizes_the_rule_actions_it_takes() {
    // A rule saved before normalization existed (or written by hand) still
    // yields a canonical plan.
    assert_eq!(
        plan_next_actions(&[], &[KICK_ALL, MODERATE]),
        Some(vec![KICK_ALL])
    );
}

#[test]
fn test_plan_identical_actions_are_a_subset_and_return_none() {
    assert_eq!(plan_next_actions(&[MODERATE], &[MODERATE]), None);
    assert_eq!(
        plan_next_actions(&[OBSERVER, MODERATE], &[OBSERVER, MODERATE]),
        None
    );
}

#[test]
fn test_smaller_action_is_subset_of_larger_action_and_returns_none() {
    // Moderation is already planned as part of the observer rule's plan.
    assert_eq!(plan_next_actions(&[OBSERVER, MODERATE], &[MODERATE]), None);
    // ...and as part of a kick that also deletes the message.
    assert_eq!(plan_next_actions(&[MODERATE, KICK], &[MODERATE]), None);
    // Deleting every message of the author's covers deleting this one.
    assert_eq!(plan_next_actions(&[KICK_ALL], &[MODERATE]), None);
}

#[test]
fn test_larger_action_covers_smaller_and_smaller_disappears() {
    // Rule 1 planned moderation; rule 2 kicks and deletes everything, which
    // subsumes it, so moderation disappears from the plan.
    assert_eq!(
        plan_next_actions(&[MODERATE], &[KICK_ALL]),
        Some(vec![KICK_ALL])
    );
}

#[test]
fn test_kick_covers_observer_and_observer_disappears() {
    assert_eq!(plan_next_actions(&[OBSERVER], &[KICK]), Some(vec![KICK]));
}

#[test]
fn test_partially_covered_plan_keeps_only_the_uncovered_part() {
    // A plain kick covers the observer role but not the message deletion, so
    // moderation survives alongside the kick.
    assert_eq!(
        plan_next_actions(&[OBSERVER, MODERATE], &[KICK]),
        Some(vec![MODERATE, KICK])
    );
}

#[test]
fn test_independent_actions_combine_in_safe_execution_order() {
    // Neither covers the other: both remain, moderation before the kick.
    assert_eq!(
        plan_next_actions(&[MODERATE], &[KICK]),
        Some(vec![MODERATE, KICK])
    );
    // And the observer role sorts ahead of both.
    assert_eq!(
        plan_next_actions(&[MODERATE], &[OBSERVER]),
        Some(vec![OBSERVER, MODERATE])
    );
}

#[test]
fn test_stronger_rule_upgrades_an_existing_plan() {
    assert_eq!(
        plan_next_actions(&[MODERATE], &[KICK_ALL]),
        Some(vec![KICK_ALL])
    );
    assert_eq!(
        plan_next_actions(&[KICK], &[KICK_ALL]),
        Some(vec![KICK_ALL])
    );
}

const OBSERVER_10M: ModerationAction = ModerationAction::SetAuthorObserver {
    duration_minutes: 10,
};
const OBSERVER_30M: ModerationAction = ModerationAction::SetAuthorObserver {
    duration_minutes: 30,
};

#[test]
fn test_longer_observer_restriction_covers_a_shorter_one() {
    assert_eq!(
        normalize_actions(&[OBSERVER_10M, OBSERVER_30M]),
        vec![OBSERVER_30M]
    );
    assert_eq!(
        normalize_actions(&[OBSERVER_30M, OBSERVER_10M]),
        vec![OBSERVER_30M]
    );
    assert_eq!(plan_next_actions(&[OBSERVER_30M], &[OBSERVER_10M]), None);
    assert_eq!(
        plan_next_actions(&[OBSERVER_10M], &[OBSERVER_30M]),
        Some(vec![OBSERVER_30M])
    );
}

#[test]
fn test_indefinite_observer_restriction_covers_every_timed_one() {
    assert_eq!(normalize_actions(&[OBSERVER_30M, OBSERVER]), vec![OBSERVER]);
    assert_eq!(plan_next_actions(&[OBSERVER], &[OBSERVER_30M]), None);
    assert_eq!(
        plan_next_actions(&[OBSERVER_30M], &[OBSERVER]),
        Some(vec![OBSERVER])
    );
}

#[test]
fn test_kick_still_covers_a_timed_observer_restriction() {
    assert_eq!(normalize_actions(&[OBSERVER_10M, KICK]), vec![KICK]);
}

/// Which action makes which redundant, for every pair: the whole relation the
/// web editor mirrors in `covered_by`, pinned so that a change to how coverage
/// is computed cannot change its outcome unnoticed.
#[test]
fn test_coverage_between_every_pair_of_actions() {
    let actions = [
        MODERATE,
        OBSERVER_10M,
        OBSERVER_30M,
        OBSERVER,
        KICK,
        KICK_ALL,
    ];
    // Row: the action already planned. Column: the action it may make redundant.
    #[rustfmt::skip]
    let expected = [
        // MODERATE OBS_10M OBS_30M OBSERVER KICK  KICK_ALL
        [true,     false,  false,  false,   false, false], // MODERATE
        [false,    true,   false,  false,   false, false], // OBSERVER_10M
        [false,    true,   true,   false,   false, false], // OBSERVER_30M
        [false,    true,   true,   true,    false, false], // OBSERVER
        [false,    true,   true,   true,    true,  false], // KICK
        [true,     true,   true,   true,    true,  true ], // KICK_ALL
    ];
    for (row, planned) in actions.iter().enumerate() {
        for (column, candidate) in actions.iter().enumerate() {
            assert_eq!(
                plan_next_actions(&[*planned], &[*candidate]).is_none(),
                expected[row][column],
                "does {planned:?} cover {candidate:?}?"
            );
        }
    }
}
