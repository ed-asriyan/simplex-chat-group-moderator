use super::*;

#[test]
fn test_matches_the_kind_it_asks_about() {
    assert_eq!(
        carries(Some(MessageAttachment::File)),
        Some("contains a file".to_string())
    );
}

#[test]
fn test_ignores_another_kind() {
    // A picture is not "a file": the owner who blocks files is not blocking
    // pictures, which have a condition of their own.
    assert_eq!(carries(Some(MessageAttachment::Image)), None);
    assert_eq!(carries(Some(MessageAttachment::Video)), None);
    assert_eq!(carries(Some(MessageAttachment::Voice)), None);
}

#[test]
fn test_plain_text_message_never_matches() {
    assert_eq!(carries(None), None);
}
