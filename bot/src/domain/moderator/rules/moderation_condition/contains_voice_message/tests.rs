use super::*;

#[test]
fn test_matches_the_kind_it_asks_about() {
    assert_eq!(
        carries(Some(MessageAttachment::Voice)),
        Some("contains a voice message".to_string())
    );
}

#[test]
fn test_ignores_another_kind() {
    assert_eq!(carries(Some(MessageAttachment::Image)), None);
    assert_eq!(carries(Some(MessageAttachment::File)), None);
}

#[test]
fn test_plain_text_message_never_matches() {
    assert_eq!(carries(None), None);
}
