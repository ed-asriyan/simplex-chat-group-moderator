use super::*;
use crate::domain::moderator::ports::ModerationRepository;
use crate::infrastructure::adapters::moderator_repo_sqlite::SqliteModerationRepository;
use crate::infrastructure::migrations;
use chrono::TimeZone;

/// A fixed instant, `minutes` past noon.
fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
}

/// A repository over a fresh database with one registered group (messenger id
/// 900), which is what a restore hangs off.
async fn repo_with_group() -> SqliteMemberRestoreRepository {
    let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    migrations::run(conn.clone()).await.unwrap();
    SqliteModerationRepository::new(conn.clone())
        .save_owner(&900, "Restore Group", &90)
        .await
        .unwrap();
    SqliteMemberRestoreRepository::new(conn)
}

#[tokio::test]
async fn test_lists_only_restores_that_are_due() {
    let repo = repo_with_group().await;
    repo.save(&900, &1, at(10)).await.unwrap();
    repo.save(&900, &2, at(30)).await.unwrap();

    let due = repo.list_due(at(20)).await.unwrap();

    assert_eq!(due.len(), 1);
    assert_eq!(due[0].messenger_group_id, 900);
    assert_eq!(due[0].member_id, 1);
    assert_eq!(due[0].execute_at, at(10));
}

#[tokio::test]
async fn test_saving_again_moves_the_members_deadline() {
    let repo = repo_with_group().await;
    repo.save(&900, &1, at(10)).await.unwrap();
    repo.save(&900, &1, at(30)).await.unwrap();

    assert!(repo.list_due(at(20)).await.unwrap().is_empty());
    let due = repo.list_due(at(40)).await.unwrap();
    assert_eq!(due.len(), 1, "the member must not be scheduled twice");
    assert_eq!(due[0].execute_at, at(30));
}

#[tokio::test]
async fn test_deleting_by_member_and_by_id() {
    let repo = repo_with_group().await;
    repo.save(&900, &1, at(10)).await.unwrap();
    repo.save(&900, &2, at(10)).await.unwrap();

    repo.delete_for_member(&900, &1).await.unwrap();
    let due = repo.list_due(at(20)).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].member_id, 2);

    repo.delete(due[0].id, due[0].execute_at).await.unwrap();
    assert!(repo.list_due(at(20)).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_leaves_a_restore_that_was_rescheduled_meanwhile() {
    let repo = repo_with_group().await;
    repo.save(&900, &1, at(10)).await.unwrap();
    let due = repo.list_due(at(20)).await.unwrap();
    // What a restriction imposed while the restore was being performed does.
    repo.save(&900, &1, at(50)).await.unwrap();

    repo.delete(due[0].id, due[0].execute_at).await.unwrap();

    let still_scheduled = repo.list_due(at(60)).await.unwrap();
    assert_eq!(still_scheduled.len(), 1);
    assert_eq!(still_scheduled[0].execute_at, at(50));
}

#[tokio::test]
async fn test_unregistered_group_has_nothing_to_restore() {
    let repo = repo_with_group().await;

    repo.save(&901, &1, at(10)).await.unwrap();

    assert!(repo.list_due(at(20)).await.unwrap().is_empty());
}
