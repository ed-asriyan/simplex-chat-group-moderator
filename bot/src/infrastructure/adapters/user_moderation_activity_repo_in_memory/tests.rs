use super::*;

#[tokio::test]
async fn test_in_memory_moderation_activity_record_and_count() {
    let repo = InMemoryUserModerationActivityRepository::new();
    let group_id = 100;
    let user_id = 200;
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    repo.record_moderated_message(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(30),
        ttl,
    )
    .await
    .unwrap();
    repo.record_moderated_message(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(10),
        ttl,
    )
    .await
    .unwrap();
    repo.record_moderated_message(&group_id, &user_id, now, ttl)
        .await
        .unwrap();

    let count_60s = repo
        .count_moderated_messages_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(60),
            now,
        )
        .await
        .unwrap();
    assert_eq!(count_60s, 3);

    let count_20s = repo
        .count_moderated_messages_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(20),
            now,
        )
        .await
        .unwrap();
    assert_eq!(count_20s, 2);
}

/// Retention is this adapter's own policy — it no longer inherits the cap by
/// delegating to the message activity repository.
#[tokio::test]
async fn test_ttl_capped_at_max_retention() {
    let repo = InMemoryUserModerationActivityRepository::new();
    let now = Utc::now();
    let msg_time = now - chrono::Duration::minutes(65);

    repo.record_moderated_message(&1, &10, msg_time, Duration::from_secs(120 * 60))
        .await
        .unwrap();

    assert_eq!(
        repo.count_moderated_messages_since(&1, &10, now - chrono::Duration::minutes(70), now)
            .await
            .unwrap(),
        0
    );
}
