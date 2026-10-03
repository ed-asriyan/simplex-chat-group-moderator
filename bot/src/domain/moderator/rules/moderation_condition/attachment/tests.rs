use super::*;

#[test]
fn matches_the_kind_it_asks_about() {
    assert_eq!(
        should_moderate(Some(MessageAttachment::Image), MessageAttachment::Image),
        Some("contains an image".to_string())
    );
    assert_eq!(
        should_moderate(Some(MessageAttachment::Voice), MessageAttachment::Voice),
        Some("contains a voice message".to_string())
    );
}

#[test]
fn ignores_another_kind() {
    assert_eq!(
        should_moderate(Some(MessageAttachment::Video), MessageAttachment::Image),
        None
    );
    // A picture is not "a file": the owner who blocks files is not blocking
    // pictures, which have a condition of their own.
    assert_eq!(
        should_moderate(Some(MessageAttachment::Image), MessageAttachment::File),
        None
    );
}

#[test]
fn plain_text_message_never_matches() {
    for wanted in [
        MessageAttachment::Image,
        MessageAttachment::Video,
        MessageAttachment::Voice,
        MessageAttachment::File,
    ] {
        assert_eq!(should_moderate(None, wanted), None);
    }
}
