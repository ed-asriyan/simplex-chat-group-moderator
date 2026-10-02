//! What belongs to the *adapter*: that a message weighs the character count it was recorded with, that the key is
//! the group alone (every member adds to the same total), and that the
//! retention cap is applied. The sliding-window mechanism is tested against the
//! driver in `drivers::sliding_window_counter`.

use super::*;

#[tokio::test]
async fn test_members_add_up_and_groups_stay_apart() {
    let repo = InMemoryGroupCharacterActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);
    let since = now - chrono::Duration::seconds(60);

    repo.record_characters(&1, now - chrono::Duration::seconds(30), 7, ttl)
        .await
        .unwrap();
    repo.record_characters(&1, now, 30, ttl).await.unwrap();
    repo.record_characters(&2, now, 500, ttl).await.unwrap();

    assert_eq!(repo.sum_characters_since(&1, since, now).await.unwrap(), 37);
    assert_eq!(
        repo.sum_characters_since(&1, now - chrono::Duration::seconds(20), now)
            .await
            .unwrap(),
        30
    );
    assert_eq!(
        repo.sum_characters_since(&2, since, now).await.unwrap(),
        500
    );
    assert_eq!(repo.sum_characters_since(&3, since, now).await.unwrap(), 0);
}

#[tokio::test]
async fn test_purge_expired_removes_inactive_groups() {
    let repo = InMemoryGroupCharacterActivityRepository::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(10);

    repo.record_characters(&1, now, 10, ttl).await.unwrap();
    repo.record_characters(&2, now, 10, ttl).await.unwrap();
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
    let repo = InMemoryGroupCharacterActivityRepository::new();
    let now = Utc::now();

    repo.record_characters(
        &1,
        now - chrono::Duration::minutes(65),
        40,
        Duration::from_secs(120 * 60),
    )
    .await
    .unwrap();

    assert_eq!(
        repo.sum_characters_since(&1, now - chrono::Duration::minutes(70), now)
            .await
            .unwrap(),
        0
    );
}
