use super::*;
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Duration as StdDuration;

#[test]
fn test_should_moderate_when_limit_reached() {
    assert_eq!(
        should_moderate(500, 500, 1),
        Some("author sent 500 characters in 1 min".to_string())
    );
    assert_eq!(
        should_moderate(1200, 500, 1),
        Some("author sent 1200 characters in 1 min".to_string())
    );
}

#[test]
fn test_should_moderate_below_limit() {
    assert_eq!(should_moderate(499, 500, 1), None);
    assert_eq!(should_moderate(0, 500, 1), None);
}

#[test]
fn test_should_moderate_zero_parameters_disabled() {
    assert_eq!(should_moderate(500, 0, 1), None);
    assert_eq!(should_moderate(500, 500, 0), None);
    assert_eq!(should_moderate(0, 0, 0), None);
}

struct MockCharacterActivityRepo {
    sum_to_return: u32,
    queried: Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, DateTime<Utc>)>>,
}

impl MockCharacterActivityRepo {
    fn new(sum_to_return: u32) -> Self {
        Self {
            sum_to_return,
            queried: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl UserCharacterActivityRepository for MockCharacterActivityRepo {
    async fn record_characters(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _timestamp: DateTime<Utc>,
        _character_count: u32,
        _ttl: StdDuration,
    ) -> Result<(), Err> {
        Ok(())
    }

    async fn sum_characters_since(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        self.queried
            .lock()
            .unwrap()
            .push((*group_id, *user_id, since, now));
        Ok(self.sum_to_return)
    }
}

#[tokio::test]
async fn test_check_triggered() {
    let repo = MockCharacterActivityRepo::new(600);
    let now = Utc::now();
    let result = check(&repo, &100, &200, 500, 2, now).await.unwrap();
    assert_eq!(
        result,
        Some("author sent 600 characters in 2 min".to_string())
    );
}

#[tokio::test]
async fn test_check_not_triggered() {
    let repo = MockCharacterActivityRepo::new(300);
    let now = Utc::now();
    let result = check(&repo, &100, &200, 500, 2, now).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_check_disabled_with_zero_parameters() {
    let repo = MockCharacterActivityRepo::new(10_000);
    let now = Utc::now();
    assert_eq!(check(&repo, &100, &200, 0, 2, now).await.unwrap(), None);
    assert_eq!(check(&repo, &100, &200, 500, 0, now).await.unwrap(), None);
    // Disabled means "never asks", not "asks and ignores the answer".
    assert!(repo.queried.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_check_queries_the_window_ending_at_now() {
    let repo = MockCharacterActivityRepo::new(0);
    let now = Utc::now();
    check(&repo, &7, &9, 500, 3, now).await.unwrap();
    let queried = repo.queried.lock().unwrap();
    assert_eq!(queried.len(), 1);
    let (group_id, user_id, since, end) = queried[0];
    assert_eq!((group_id, user_id), (7, 9));
    assert_eq!(since, now - Duration::minutes(3));
    assert_eq!(end, now);
}
