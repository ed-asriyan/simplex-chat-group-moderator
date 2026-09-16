use super::*;

#[test]
fn matches_the_kind_it_asks_about() {
    assert_eq!(
        should_moderate(
            Some(MessageAttachment::Image),
            &ModerationCondition::ContainsImage
        ),
        Some("contains an image".to_string())
    );
    assert_eq!(
        should_moderate(
            Some(MessageAttachment::Voice),
            &ModerationCondition::ContainsVoiceMessage
        ),
        Some("contains a voice message".to_string())
    );
}

#[test]
fn ignores_another_kind() {
    assert_eq!(
        should_moderate(
            Some(MessageAttachment::Video),
            &ModerationCondition::ContainsImage
        ),
        None
    );
    // A picture is not "a file": the owner who blocks files is not blocking
    // pictures, which have a condition of their own.
    assert_eq!(
        should_moderate(
            Some(MessageAttachment::Image),
            &ModerationCondition::ContainsFile
        ),
        None
    );
}

#[test]
fn plain_text_message_never_matches() {
    for condition in [
        ModerationCondition::ContainsImage,
        ModerationCondition::ContainsVideo,
        ModerationCondition::ContainsVoiceMessage,
        ModerationCondition::ContainsFile,
    ] {
        assert_eq!(should_moderate(None, &condition), None);
    }
}

#[test]
fn non_attachment_condition_never_matches() {
    assert_eq!(
        should_moderate(
            Some(MessageAttachment::Image),
            &ModerationCondition::IsBlank
        ),
        None
    );
}
