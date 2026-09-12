use super::MemberRestoreApplication;
use crate::domain::moderator::application::tests::{
    MockGroupModerator, MockMemberRestoreRepository,
};
use crate::domain::moderator::ports::{MemberRestoreRunner, ScheduledMemberRestore};
use chrono::{Duration, TimeZone, Utc};
use std::sync::Arc;

fn restore(id: i64, member_id: i64, execute_at_minutes: i64) -> ScheduledMemberRestore {
    ScheduledMemberRestore {
        id,
        messenger_group_id: 7,
        member_id,
        execute_at: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
            + Duration::minutes(execute_at_minutes),
    }
}

fn now(offset_minutes: i64) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap() + Duration::minutes(offset_minutes)
}

fn app(
    restores: Arc<MockMemberRestoreRepository>,
    moderator: Arc<MockGroupModerator>,
) -> MemberRestoreApplication {
    MemberRestoreApplication::new(restores, moderator)
}

#[tokio::test]
async fn test_restores_members_whose_time_is_up_and_forgets_them() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    restores.due.lock().unwrap().push(restore(1, 100, 5));
    let moderator = Arc::new(MockGroupModerator::default());

    app(restores.clone(), moderator.clone())
        .run_due_restores(now(5))
        .await
        .unwrap();

    assert_eq!(*moderator.restored_members.lock().unwrap(), vec![(7, 100)]);
    assert_eq!(*restores.deleted.lock().unwrap(), vec![1]);
}

#[tokio::test]
async fn test_leaves_members_whose_time_has_not_come() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    restores.due.lock().unwrap().push(restore(1, 100, 30));
    let moderator = Arc::new(MockGroupModerator::default());

    app(restores.clone(), moderator.clone())
        .run_due_restores(now(29))
        .await
        .unwrap();

    assert!(moderator.restored_members.lock().unwrap().is_empty());
    assert!(restores.deleted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_failed_restore_keeps_its_row_for_the_next_run() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    restores.due.lock().unwrap().push(restore(1, 100, 5));
    let moderator = Arc::new(MockGroupModerator {
        fail_role_change: true,
        ..Default::default()
    });

    let err = app(restores.clone(), moderator)
        .run_due_restores(now(5))
        .await
        .expect_err("a failing messenger should be reported");

    assert!(err.to_string().contains("role change failed"));
    assert!(
        restores.deleted.lock().unwrap().is_empty(),
        "a restore that did not happen must stay scheduled"
    );
}

#[tokio::test]
async fn test_restores_every_due_member() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    restores.due.lock().unwrap().push(restore(1, 100, 1));
    restores.due.lock().unwrap().push(restore(2, 200, 2));
    let moderator = Arc::new(MockGroupModerator::default());

    app(restores.clone(), moderator.clone())
        .run_due_restores(now(10))
        .await
        .unwrap();

    assert_eq!(
        *moderator.restored_members.lock().unwrap(),
        vec![(7, 100), (7, 200)]
    );
    assert_eq!(*restores.deleted.lock().unwrap(), vec![1, 2]);
}

#[tokio::test]
async fn test_gives_up_on_a_restore_that_has_been_failing_for_a_day() {
    let restores = Arc::new(MockMemberRestoreRepository::default());
    restores.due.lock().unwrap().push(restore(1, 100, 0));
    let moderator = Arc::new(MockGroupModerator {
        fail_role_change: true,
        ..Default::default()
    });

    app(restores.clone(), moderator.clone())
        .run_due_restores(now(24 * 60 + 1))
        .await
        .expect_err("the failure is still reported");

    assert_eq!(
        *restores.deleted.lock().unwrap(),
        vec![1],
        "a member the bot can never restore must not be retried forever"
    );
}

#[tokio::test]
async fn test_a_failed_delete_does_not_hold_up_the_other_members() {
    let restores = Arc::new(MockMemberRestoreRepository {
        fail_writes: true,
        ..Default::default()
    });
    restores.due.lock().unwrap().push(restore(1, 100, 1));
    restores.due.lock().unwrap().push(restore(2, 200, 2));
    let moderator = Arc::new(MockGroupModerator::default());

    app(restores.clone(), moderator.clone())
        .run_due_restores(now(10))
        .await
        .expect_err("the failure is reported");

    assert_eq!(
        *moderator.restored_members.lock().unwrap(),
        vec![(7, 100), (7, 200)],
        "both members must have been restored despite the first delete failing"
    );
}
