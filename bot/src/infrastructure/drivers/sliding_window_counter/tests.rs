use super::*;

/// The counter is generic over its key; these tests use the shape the adapters
/// use, a (group, user) pair.
type Counter = SlidingWindowCounter<(i64, i64)>;

#[test]
fn test_total_over_the_window() {
    let counter = Counter::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    counter
        .add((100, 200), now - chrono::Duration::seconds(30), 1, ttl)
        .unwrap();
    counter
        .add((100, 200), now - chrono::Duration::seconds(10), 1, ttl)
        .unwrap();
    counter.add((100, 200), now, 1, ttl).unwrap();

    assert_eq!(
        counter
            .total(&(100, 200), now - chrono::Duration::seconds(60), now)
            .unwrap(),
        3
    );
    assert_eq!(
        counter
            .total(&(100, 200), now - chrono::Duration::seconds(20), now)
            .unwrap(),
        2
    );
    // A window that starts after the last entry holds nothing.
    assert_eq!(
        counter
            .total(&(100, 200), now + chrono::Duration::seconds(10), now)
            .unwrap(),
        0
    );
}

/// The amount is what separates "how many messages" from "how many characters":
/// the same three entries weigh 3 or 165 depending on what was added.
#[test]
fn test_total_sums_amounts_not_entries() {
    let counter = Counter::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    counter.add((1, 10), now, 100, ttl).unwrap();
    counter.add((1, 10), now, 60, ttl).unwrap();
    counter.add((1, 10), now, 5, ttl).unwrap();

    assert_eq!(
        counter
            .total(&(1, 10), now - chrono::Duration::seconds(60), now)
            .unwrap(),
        165
    );
}

#[test]
fn test_total_saturates_instead_of_wrapping() {
    let counter = Counter::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);

    counter.add((1, 10), now, u32::MAX, ttl).unwrap();
    counter.add((1, 10), now, 10, ttl).unwrap();

    assert_eq!(
        counter
            .total(&(1, 10), now - chrono::Duration::seconds(60), now)
            .unwrap(),
        u32::MAX
    );
}

#[test]
fn test_keys_are_isolated() {
    let counter = Counter::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(600);
    let since = now - chrono::Duration::seconds(60);

    counter.add((1, 10), now, 1, ttl).unwrap();
    counter.add((1, 20), now, 1, ttl).unwrap();
    counter.add((1, 20), now, 1, ttl).unwrap();
    counter.add((2, 10), now, 1, ttl).unwrap();

    assert_eq!(counter.total(&(1, 10), since, now).unwrap(), 1);
    assert_eq!(counter.total(&(1, 20), since, now).unwrap(), 2);
    assert_eq!(counter.total(&(2, 10), since, now).unwrap(), 1);
    assert_eq!(counter.total(&(2, 20), since, now).unwrap(), 0);
}

#[test]
fn test_expired_entry_is_not_counted() {
    let counter = Counter::new();
    let now = Utc::now();
    let at = now - chrono::Duration::seconds(10);

    // Expired five seconds ago, even though it falls inside the window asked for.
    counter.add((1, 10), at, 1, Duration::from_secs(5)).unwrap();

    assert_eq!(
        counter
            .total(&(1, 10), now - chrono::Duration::seconds(20), now)
            .unwrap(),
        0
    );
}

#[test]
fn test_emptied_key_is_dropped_on_read() {
    let counter = Counter::new();
    let now = Utc::now();
    let at = now - chrono::Duration::seconds(10);

    counter.add((1, 10), at, 1, Duration::from_secs(5)).unwrap();
    assert_eq!(counter.active_key_count().unwrap(), 1);

    counter
        .total(&(1, 10), now - chrono::Duration::seconds(20), now)
        .unwrap();
    assert_eq!(counter.active_key_count().unwrap(), 0);
}

#[test]
fn test_purge_expired_drops_only_what_expired() {
    let counter = Counter::new();
    let now = Utc::now();
    let ttl = Duration::from_secs(10);

    counter.add((1, 101), now, 1, ttl).unwrap();
    counter.add((1, 102), now, 1, ttl).unwrap();
    counter.add((1, 103), now, 1, ttl).unwrap();
    assert_eq!(counter.active_key_count().unwrap(), 3);

    assert_eq!(
        counter
            .purge_expired(now + chrono::Duration::seconds(5))
            .unwrap(),
        0
    );
    assert_eq!(counter.active_key_count().unwrap(), 3);

    assert_eq!(
        counter
            .purge_expired(now + chrono::Duration::seconds(15))
            .unwrap(),
        3
    );
    assert_eq!(counter.active_key_count().unwrap(), 0);
}

#[test]
fn test_adding_sweeps_keys_nobody_asks_about() {
    let counter = Counter::new();
    let base = Utc::now() - chrono::Duration::hours(2);
    let ttl = Duration::from_secs(600);

    counter.add((1, 101), base, 1, ttl).unwrap();
    counter.add((1, 102), base, 1, ttl).unwrap();
    counter.add((1, 103), base, 1, ttl).unwrap();
    assert_eq!(counter.active_key_count().unwrap(), 3);

    // Two hours on, the periodic sweep runs as part of the add.
    counter.add((1, 104), Utc::now(), 1, ttl).unwrap();
    assert_eq!(counter.active_key_count().unwrap(), 1);
}

/// The counter keeps exactly the ttl it is handed — capping it is the
/// adapter's policy, and a counter that quietly shortened it would make that
/// policy impossible to check from the adapter.
#[test]
fn test_ttl_is_honoured_as_given() {
    let counter = Counter::new();
    let now = Utc::now();
    let at = now - chrono::Duration::minutes(65);

    counter
        .add((1, 10), at, 1, Duration::from_secs(120 * 60))
        .unwrap();

    assert_eq!(
        counter
            .total(&(1, 10), now - chrono::Duration::minutes(70), now)
            .unwrap(),
        1
    );
}
