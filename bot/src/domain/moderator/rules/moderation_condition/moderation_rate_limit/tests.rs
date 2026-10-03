use super::*;
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Duration as StdDuration;

#[test]
fn test_should_moderate_when_limit_reached() {
    assert_eq!(
        should_moderate(3, 3, 60),
        Some("author had 3 messages moderated in 60 min".to_string())
    );
    assert_eq!(
        should_moderate(5, 3, 60),
        Some("author had 5 messages moderated in 60 min".to_string())
    );
}

#[test]
fn test_should_moderate_below_limit() {
    assert_eq!(should_moderate(2, 3, 60), None);
    assert_eq!(should_moderate(0, 3, 60), None);
}

#[test]
fn test_should_moderate_zero_parameters_disabled() {
    assert_eq!(should_moderate(5, 0, 60), None);
    assert_eq!(should_moderate(5, 3, 0), None);
    assert_eq!(should_moderate(0, 0, 0), None);
}

struct MockModerationActivityRepo {
    count_to_return: u32,
    recorded: Mutex<Vec<(MessengerGroupId, UserId, DateTime<Utc>, StdDuration)>>,
}

#[async_trait]
impl UserModerationActivityRepository for MockModerationActivityRepo {
    async fn record_moderated_message(
        &self,
        group_id: &MessengerGroupId,
        user_id: &UserId,
        timestamp: DateTime<Utc>,
        ttl: StdDuration,
    ) -> Result<(), Err> {
        self.recorded
            .lock()
            .unwrap()
            .push((*group_id, *user_id, timestamp, ttl));
        Ok(())
    }

    async fn count_moderated_messages_since(
        &self,
        _group_id: &MessengerGroupId,
        _user_id: &UserId,
        _since: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<u32, Err> {
        Ok(self.count_to_return)
    }
}

#[tokio::test]
async fn test_check_triggered() {
    let repo = MockModerationActivityRepo {
        count_to_return: 3,
        recorded: Mutex::new(Vec::new()),
    };
    let now = Utc::now();
    let result = check(&repo, &100, &200, 3, 60, now).await.unwrap();
    assert_eq!(
        result,
        Some("author had 3 messages moderated in 60 min".to_string())
    );
}

#[tokio::test]
async fn test_check_not_triggered() {
    let repo = MockModerationActivityRepo {
        count_to_return: 2,
        recorded: Mutex::new(Vec::new()),
    };
    let now = Utc::now();
    let result = check(&repo, &100, &200, 3, 60, now).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_check_disabled_with_zero_limit() {
    let repo = MockModerationActivityRepo {
        count_to_return: 10,
        recorded: Mutex::new(Vec::new()),
    };
    let now = Utc::now();
    let result = check(&repo, &100, &200, 0, 60, now).await.unwrap();
    assert_eq!(result, None);
}
