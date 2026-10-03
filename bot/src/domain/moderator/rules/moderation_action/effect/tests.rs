use super::*;

#[test]
fn test_silence_orders_from_none_through_timed_to_forever() {
    assert!(Silence::No < Silence::For { minutes: 1 });
    assert!(Silence::For { minutes: 10 } < Silence::For { minutes: 30 });
    assert!(Silence::For { minutes: u32::MAX } < Silence::Forever);
}

#[test]
fn test_an_effect_covers_itself_and_anything_weaker() {
    let silenced = Effect {
        author_silenced: Silence::For { minutes: 10 },
        ..Effect::default()
    };
    assert!(silenced.covers(&silenced));
    assert!(silenced.covers(&Effect::default()));
    assert!(!Effect::default().covers(&silenced));
}

#[test]
fn test_an_effect_must_achieve_at_least_as_much_on_every_count() {
    let deleted = Effect {
        message_deleted: true,
        ..Effect::default()
    };
    let removed = Effect {
        author_silenced: Silence::Forever,
        author_removed: true,
        ..Effect::default()
    };
    assert!(!deleted.covers(&removed));
    assert!(!removed.covers(&deleted));
}
