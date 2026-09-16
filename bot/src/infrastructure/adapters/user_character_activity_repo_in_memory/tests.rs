//! What belongs to the *adapter*: that a message weighs its character count,
//! that the key is the (group, user) pair, and that the retention cap is
//! applied. The sliding-window mechanism is tested against the driver in
//! `drivers::sliding_window_counter`.

use super::*;

#[tokio::test]
async fn test_record_and_sum() {
    let repo = InMemoryUserCharacterActivityRepository::new();
    let group_id = 100;
    let user_id = 200;
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    repo.record_characters(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(30),
        100,
        ttl,
    )
    .await
    .unwrap();
    repo.record_characters(
        &group_id,
        &user_id,
        now - chrono::Duration::seconds(10),
        60,
        ttl,
    )
    .await
    .unwrap();
    repo.record_characters(&group_id, &user_id, now, 5, ttl)
        .await
        .unwrap();

    // Three messages, but this port counts what was written, not how often.
    assert_eq!(
        repo.sum_characters_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(60),
            now
        )
        .await
        .unwrap(),
        165
    );
    assert_eq!(
        repo.sum_characters_since(
            &group_id,
            &user_id,
            now - chrono::Duration::seconds(20),
            now
        )
        .await
        .unwrap(),
        65
    );
}

#[tokio::test]
async fn test_isolation_between_users_and_groups() {
    let repo = InMemoryUserCharacterActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);
    let since = now - chrono::Duration::seconds(60);

    repo.record_characters(&1, &10, now, 7, ttl).await.unwrap();
    repo.record_characters(&1, &20, now, 30, ttl).await.unwrap();
    repo.record_characters(&2, &10, now, 500, ttl)
        .await
        .unwrap();

    assert_eq!(
        repo.sum_characters_since(&1, &10, since, now)
            .await
            .unwrap(),
        7
    );
    assert_eq!(
        repo.sum_characters_since(&1, &20, since, now)
            .await
            .unwrap(),
        30
    );
    assert_eq!(
        repo.sum_characters_since(&2, &10, since, now)
            .await
            .unwrap(),
        500
    );
    assert_eq!(
        repo.sum_characters_since(&2, &20, since, now)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn test_purge_expired_removes_inactive_users() {
    let repo = InMemoryUserCharacterActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(10);

    repo.record_characters(&1, &101, now, 10, ttl)
        .await
        .unwrap();
    repo.record_characters(&1, &102, now, 10, ttl)
        .await
        .unwrap();
    assert_eq!(repo.active_key_count(), 2);

    let removed = repo
        .purge_expired(now + chrono::Duration::seconds(15))
        .unwrap();
    assert_eq!(removed, 2);
    assert_eq!(repo.active_key_count(), 0);
}

/// Retention is this adapter's policy: the counter keeps whatever TTL it is
/// handed, so a caller asking for two hours must still get 60 minutes.
#[tokio::test]
async fn test_ttl_capped_at_max_retention() {
    let repo = InMemoryUserCharacterActivityRepository::new();
    let now = Utc::now();
    let msg_time = now - chrono::Duration::minutes(65);

    repo.record_characters(&1, &10, msg_time, 500, Duration::from_secs(120 * 60))
        .await
        .unwrap();

    assert_eq!(
        repo.sum_characters_since(&1, &10, now - chrono::Duration::minutes(70), now)
            .await
            .unwrap(),
        0
    );
}
