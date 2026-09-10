use super::planner::*;
use crate::domain::moderator::ports::{
    DeleteAuthorMessages, DeleteObserverMessages, ModerationAction,
};

#[test]
fn test_plan_from_empty_moderate_message() {
    let planned = plan_next_actions(&[], &ModerationAction::ModerateMessage);
    assert_eq!(planned, Some(vec![PlannedAction::DeleteTriggeredMessage]));
}

#[test]
fn test_plan_from_empty_set_observer_with_triggered() {
    let planned = plan_next_actions(
        &[],
        &ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::TriggeredMessage,
        },
    );
    assert_eq!(
        planned,
        Some(vec![
            PlannedAction::SetObserver,
            PlannedAction::DeleteTriggeredMessage
        ])
    );
}

#[test]
fn test_plan_from_empty_set_observer_with_none() {
    let planned = plan_next_actions(
        &[],
        &ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::None,
        },
    );
    assert_eq!(planned, Some(vec![PlannedAction::SetObserver]));
}

#[test]
fn test_plan_from_empty_kick_author_none() {
    let planned = plan_next_actions(
        &[],
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
    );
    assert_eq!(
        planned,
        Some(vec![PlannedAction::KickAuthor {
            delete_all_messages: false
        }])
    );
}

#[test]
fn test_plan_from_empty_kick_author_triggered() {
    let planned = plan_next_actions(
        &[],
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage,
        },
    );
    assert_eq!(
        planned,
        Some(vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ])
    );
}

#[test]
fn test_plan_from_empty_kick_author_all_messages() {
    let planned = plan_next_actions(
        &[],
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        },
    );
    assert_eq!(
        planned,
        Some(vec![PlannedAction::KickAuthor {
            delete_all_messages: true
        }])
    );
}

#[test]
fn test_plan_identical_action_is_subset_and_returns_none() {
    let current = vec![PlannedAction::DeleteTriggeredMessage];
    assert_eq!(
        plan_next_actions(&current, &ModerationAction::ModerateMessage),
        None
    );

    let observer_current = vec![
        PlannedAction::SetObserver,
        PlannedAction::DeleteTriggeredMessage,
    ];
    assert_eq!(
        plan_next_actions(
            &observer_current,
            &ModerationAction::SetAuthorObserver {
                delete_message: DeleteObserverMessages::TriggeredMessage
            }
        ),
        None
    );
}

#[test]
fn test_smaller_action_is_subset_of_larger_action_and_returns_none() {
    // ModerateMessage { DeleteTriggeredMessage } is a subset of SetAuthorObserver { SetObserver, DeleteTriggeredMessage }
    let current_observer = vec![
        PlannedAction::SetObserver,
        PlannedAction::DeleteTriggeredMessage,
    ];
    assert_eq!(
        plan_next_actions(&current_observer, &ModerationAction::ModerateMessage),
        None
    );

    // ModerateMessage { DeleteTriggeredMessage } is a subset of KickAuthor { KickMember, DeleteTriggeredMessage }
    let current_kick = vec![
        PlannedAction::DeleteTriggeredMessage,
        PlannedAction::KickAuthor {
            delete_all_messages: false,
        },
    ];
    assert_eq!(
        plan_next_actions(&current_kick, &ModerationAction::ModerateMessage),
        None
    );

    // ModerateMessage { DeleteTriggeredMessage } is covered by KickAuthor { KickMember, DeleteAllMessages }
    // because DeleteAllMessages covers DeleteTriggeredMessage
    let current_kick_all = vec![PlannedAction::KickAuthor {
        delete_all_messages: true,
    }];
    assert_eq!(
        plan_next_actions(&current_kick_all, &ModerationAction::ModerateMessage),
        None
    );
}

#[test]
fn test_larger_action_covers_smaller_and_smaller_disappears() {
    // Rule 1: ModerateMessage -> [DeleteTriggeredMessage]
    // Rule 2: KickAuthor { delete_messages: AllMessages }
    // Since KickAuthor { AllMessages } covers DeleteTriggeredMessage, DeleteTriggeredMessage disappears!
    let current = vec![PlannedAction::DeleteTriggeredMessage];
    let next = plan_next_actions(
        &current,
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages,
        },
    );
    assert_eq!(
        next,
        Some(vec![PlannedAction::KickAuthor {
            delete_all_messages: true
        }])
    );
}

#[test]
fn test_kick_covers_observer_and_observer_disappears() {
    // Rule 1: SetAuthorObserver { delete_message: None } -> [SetObserver]
    // Rule 2: KickAuthor { delete_messages: None } -> [KickMember]
    // KickMember covers SetObserver, so SetObserver disappears!
    let current = vec![PlannedAction::SetObserver];
    let next = plan_next_actions(
        &current,
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
    );
    assert_eq!(
        next,
        Some(vec![PlannedAction::KickAuthor {
            delete_all_messages: false
        }])
    );
}

#[test]
fn test_partially_covered_action_removes_only_covered_part() {
    // Current: SetAuthorObserver { delete_message: TriggeredMessage } -> { SetObserver, DeleteTriggeredMessage }
    // Rule 2: KickAuthor { delete_messages: None } -> { KickMember }
    // KickMember covers SetObserver, so SetObserver disappears.
    // But DeleteTriggeredMessage is NOT covered by KickMember { None }, so DeleteTriggeredMessage remains!
    let current = vec![
        PlannedAction::SetObserver,
        PlannedAction::DeleteTriggeredMessage,
    ];
    let next = plan_next_actions(
        &current,
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
    );
    assert_eq!(
        next,
        Some(vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ])
    );
}

#[test]
fn test_independent_actions_combine_in_safe_execution_order() {
    // Rule 1: ModerateMessage -> [DeleteTriggeredMessage]
    // Rule 2: KickAuthor { delete_messages: None } -> [KickAuthor]
    // Neither covers the other! Both remain, and DeleteTriggeredMessage MUST run before KickAuthor.
    let current = vec![PlannedAction::DeleteTriggeredMessage];
    let next = plan_next_actions(
        &current,
        &ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None,
        },
    );
    assert_eq!(
        next,
        Some(vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ])
    );
}

#[test]
fn test_subset_and_covers_predicates() {
    // ModerateMessage is a subset of KickAuthor { AllMessages }: adding it changes nothing.
    assert_eq!(
        plan_next_actions(
            &[PlannedAction::KickAuthor {
                delete_all_messages: true
            }],
            &ModerationAction::ModerateMessage
        ),
        None
    );

    // KickAuthor { AllMessages } is NOT a subset of ModerateMessage: it must upgrade the plan.
    assert_eq!(
        plan_next_actions(
            &[PlannedAction::DeleteTriggeredMessage],
            &ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::AllMessages
            }
        ),
        Some(vec![PlannedAction::KickAuthor {
            delete_all_messages: true
        }])
    );

    // KickAuthor { None } does not cover ModerateMessage (it does not delete the message),
    // so the deletion survives alongside the kick.
    assert_eq!(
        plan_next_actions(
            &[PlannedAction::DeleteTriggeredMessage],
            &ModerationAction::KickAuthor {
                delete_messages: DeleteAuthorMessages::None
            }
        ),
        Some(vec![
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            },
        ])
    );
}

#[test]
fn test_planned_actions_to_moderation_action_conversion() {
    assert_eq!(
        planned_actions_to_moderation_action(&[PlannedAction::DeleteTriggeredMessage]),
        ModerationAction::ModerateMessage
    );

    assert_eq!(
        planned_actions_to_moderation_action(&[PlannedAction::SetObserver]),
        ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::None
        }
    );

    assert_eq!(
        planned_actions_to_moderation_action(&[
            PlannedAction::SetObserver,
            PlannedAction::DeleteTriggeredMessage
        ]),
        ModerationAction::SetAuthorObserver {
            delete_message: DeleteObserverMessages::TriggeredMessage
        }
    );

    assert_eq!(
        planned_actions_to_moderation_action(&[PlannedAction::KickAuthor {
            delete_all_messages: false
        }]),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::None
        }
    );

    assert_eq!(
        planned_actions_to_moderation_action(&[
            PlannedAction::DeleteTriggeredMessage,
            PlannedAction::KickAuthor {
                delete_all_messages: false
            }
        ]),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::TriggeredMessage
        }
    );

    assert_eq!(
        planned_actions_to_moderation_action(&[PlannedAction::KickAuthor {
            delete_all_messages: true
        }]),
        ModerationAction::KickAuthor {
            delete_messages: DeleteAuthorMessages::AllMessages
        }
    );
}
