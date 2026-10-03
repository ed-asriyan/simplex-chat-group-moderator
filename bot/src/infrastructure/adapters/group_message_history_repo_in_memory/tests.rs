//! What belongs to the adapter: per-group order, the `keep` window, edits in
//! place, forgetting, and its own retention caps.

use super::*;
use chrono::Duration;

fn message(message_id: MessageId, at: DateTime<Utc>) -> RecentGroupMessage {
    RecentGroupMessage {
        message_id,
        author_id: 1,
        author_name: "Alice".to_string(),
        text: format!("message {message_id}"),
        attachment: None,
        timestamp: at,
    }
}

fn ids(messages: &[RecentGroupMessage]) -> Vec<MessageId> {
    messages.iter().map(|m| m.message_id).collect()
}

#[tokio::test]
async fn test_latest_messages_come_back_oldest_first_and_groups_stay_apart() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=4 {
        repo.record_message(&1, message(id, now), 10).await.unwrap();
    }
    repo.record_message(&2, message(9, now), 10).await.unwrap();

    // A new message is not kept yet: its context is the latest `count`.
    assert_eq!(
        ids(&repo.messages_before(&1, &5, 2, now).await.unwrap()),
        vec![3, 4]
    );
    assert_eq!(
        ids(&repo.messages_before(&1, &5, 10, now).await.unwrap()),
        vec![1, 2, 3, 4]
    );
    assert_eq!(
        ids(&repo.messages_before(&2, &5, 10, now).await.unwrap()),
        vec![9]
    );
    assert!(
        repo.messages_before(&3, &5, 10, now)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn test_only_keep_latest_are_kept() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=5 {
        repo.record_message(&1, message(id, now), 3).await.unwrap();
    }
    assert_eq!(
        ids(&repo.messages_before(&1, &6, 10, now).await.unwrap()),
        vec![3, 4, 5]
    );
}

#[tokio::test]
async fn test_keep_is_capped() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=(MAX_KEPT_PER_GROUP as i64 + 5) {
        repo.record_message(&1, message(id, now), u32::MAX)
            .await
            .unwrap();
    }
    let kept = repo.messages_before(&1, &0, u32::MAX, now).await.unwrap();
    assert_eq!(kept.len(), MAX_KEPT_PER_GROUP as usize);
}

#[tokio::test]
async fn test_an_edited_message_reads_only_what_came_before_it() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=4 {
        repo.record_message(&1, message(id, now), 10).await.unwrap();
    }
    assert_eq!(
        ids(&repo.messages_before(&1, &3, 10, now).await.unwrap()),
        vec![1, 2]
    );
}

#[tokio::test]
async fn test_an_edit_replaces_the_kept_message_in_place() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=3 {
        repo.record_message(&1, message(id, now), 10).await.unwrap();
    }
    let mut edit = message(2, now);
    edit.text = "edited".to_string();
    repo.record_edit(&1, edit).await.unwrap();
    // An edit of a message no longer kept does not come back.
    repo.record_edit(&1, message(99, now)).await.unwrap();

    let kept = repo.messages_before(&1, &0, 10, now).await.unwrap();
    assert_eq!(ids(&kept), vec![1, 2, 3]);
    assert_eq!(kept[1].text, "edited");
}

#[tokio::test]
async fn test_a_forgotten_message_is_gone() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    for id in 1..=3 {
        repo.record_message(&1, message(id, now), 10).await.unwrap();
    }
    repo.forget_message(&1, &2).await.unwrap();
    assert_eq!(
        ids(&repo.messages_before(&1, &0, 10, now).await.unwrap()),
        vec![1, 3]
    );
}

#[tokio::test]
async fn test_messages_past_retention_are_not_returned_and_are_purged() {
    let repo = InMemoryGroupMessageHistoryRepository::new();
    let now = Utc::now();
    repo.record_message(
        &1,
        message(1, now - MAX_RETENTION - Duration::minutes(1)),
        10,
    )
    .await
    .unwrap();
    repo.record_message(&2, message(2, now - Duration::hours(1)), 10)
        .await
        .unwrap();

    assert!(
        repo.messages_before(&1, &0, 10, now)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(repo.active_group_count(), 2);

    assert_eq!(repo.purge_expired(now).unwrap(), 1);
    assert_eq!(repo.active_group_count(), 1);
    assert_eq!(
        ids(&repo.messages_before(&2, &0, 10, now).await.unwrap()),
        vec![2]
    );
}
