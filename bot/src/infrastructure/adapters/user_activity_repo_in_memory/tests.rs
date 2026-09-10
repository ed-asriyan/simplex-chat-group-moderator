use super::*;

#[tokio::test]
async fn test_in_memory_record_and_count() {
    let repo = InMemoryUserActivityRepository::new();
    let group_id = 100;
    let user_id = 200;
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    repo.record_user_message(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(30),
        ttl,
    )
    .await
    .unwrap();
    repo.record_user_message(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(10),
        ttl,
    )
    .await
    .unwrap();
    repo.record_user_message(&group_id, &user_id, now, ttl)
        .await
        .unwrap();

    // Total count in last 60 seconds should be 3
    let count_60s = repo
        .count_messages_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(60),
            now,
        )
        .await
        .unwrap();
    assert_eq!(count_60s, 3);

    // Count in last 20 seconds should be 2
    let count_20s = repo
        .count_messages_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(20),
            now,
        )
        .await
        .unwrap();
    assert_eq!(count_20s, 2);

    // Count in future should be 0
    let count_future = repo
        .count_messages_since(
            &group_id,
            &user_id,
            now + chrono::Duration::seconds(10),
            now,
        )
        .await
        .unwrap();
    assert_eq!(count_future, 0);
}

#[tokio::test]
async fn test_in_memory_isolation_between_users_and_groups() {
    let repo = InMemoryUserActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    // User 1 in Group 1
    repo.record_user_message(&1, &10, now, ttl).await.unwrap();
    // User 2 in Group 1
    repo.record_user_message(&1, &20, now, ttl).await.unwrap();
    repo.record_user_message(&1, &20, now, ttl).await.unwrap();
    // User 1 in Group 2
    repo.record_user_message(&2, &10, now, ttl).await.unwrap();

    let since = now - chrono::Duration::seconds(10);
    assert_eq!(
        repo.count_messages_since(&1, &10, since, now)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo.count_messages_since(&1, &20, since, now)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        repo.count_messages_since(&2, &10, since, now)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo.count_messages_since(&2, &20, since, now)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn test_in_memory_ttl_expiration() {
    let repo = InMemoryUserActivityRepository::new();
    let now = Utc::now();
    // Record message with very short TTL in the past
    let timestamp = now - chrono::Duration::seconds(10);
    let ttl = Duration::from_secs(5); // expired 5 seconds ago

    repo.record_user_message(&1, &10, timestamp, ttl)
        .await
        .unwrap();

    let since = now - chrono::Duration::seconds(20);
    assert_eq!(
        repo.count_messages_since(&1, &10, since, now)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn test_empty_user_entry_removed_on_count() {
    let repo = InMemoryUserActivityRepository::new();
    let now = Utc::now();
    let timestamp = now - chrono::Duration::seconds(10);
    let ttl = Duration::from_secs(5);

    repo.record_user_message(&1, &10, timestamp, ttl)
        .await
        .unwrap();
    assert_eq!(repo.active_key_count(), 1);

    // When counting messages at `now`, the expired message is evicted and the key is removed from the map
    let count = repo
        .count_messages_since(&1, &10, now - chrono::Duration::seconds(20), now)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(repo.active_key_count(), 0);
}

#[tokio::test]
async fn test_purge_expired_removes_inactive_users() {
    let repo = InMemoryUserActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(10);

    // 3 users send messages with 10-second TTL
    repo.record_user_message(&1, &101, now, ttl).await.unwrap();
    repo.record_user_message(&1, &102, now, ttl).await.unwrap();
    repo.record_user_message(&1, &103, now, ttl).await.unwrap();
    assert_eq!(repo.active_key_count(), 3);

    // 5 seconds later: not yet expired
    let removed = repo
        .purge_expired(now + chrono::Duration::seconds(5))
        .unwrap();
    assert_eq!(removed, 0);
    assert_eq!(repo.active_key_count(), 3);

    // 15 seconds later: all 3 expired and purged
    let removed = repo
        .purge_expired(now + chrono::Duration::seconds(15))
        .unwrap();
    assert_eq!(removed, 3);
    assert_eq!(repo.active_key_count(), 0);
}

#[tokio::test]
async fn test_automatic_sweep_on_record_message() {
    let repo = InMemoryUserActivityRepository::new();
    let base = Utc::now() - chrono::Duration::hours(2);
    let ttl = Duration::from_secs(600);

    // 3 users send messages 2 hours ago
    repo.record_user_message(&1, &101, base, ttl).await.unwrap();
    repo.record_user_message(&1, &102, base, ttl).await.unwrap();
    repo.record_user_message(&1, &103, base, ttl).await.unwrap();
    assert_eq!(repo.active_key_count(), 3);

    // New message 2 hours later automatically triggers the periodic sweep
    let now = Utc::now();
    repo.record_user_message(&1, &104, now, ttl).await.unwrap();

    // Inactive users were automatically purged during record_user_message; only user 104 remains
    assert_eq!(repo.active_key_count(), 1);
}

#[tokio::test]
async fn test_ttl_capped_at_max_60_minutes() {
    let repo = InMemoryUserActivityRepository::new();
    let now = Utc::now();
    let msg_time = now - chrono::Duration::minutes(65);

    // Request TTL of 2 hours (exceeding 60-min cap)
    repo.record_user_message(&1, &10, msg_time, Duration::from_secs(120 * 60))
        .await
        .unwrap();

    // Since effective TTL is capped at 60 min, at `now` (65 min later) it must be expired!
    let count = repo
        .count_messages_since(&1, &10, now - chrono::Duration::minutes(70), now)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
