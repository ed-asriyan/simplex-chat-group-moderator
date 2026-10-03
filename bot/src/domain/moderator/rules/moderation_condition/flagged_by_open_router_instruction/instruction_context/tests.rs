use super::*;
use crate::domain::moderator::ports::{MessageAttachment, UserId};
use chrono::Utc;

fn message(author_id: UserId, text: &str) -> RecentGroupMessage {
    RecentGroupMessage {
        message_id: 0,
        author_id,
        author_name: format!("name {author_id}"),
        text: text.to_string(),
        attachment: None,
        timestamp: Utc::now(),
    }
}

#[test]
fn test_order_names_and_text_are_kept() {
    let context = for_model(&[message(1, "first"), message(2, "second")]);
    let shown: Vec<(&str, &str)> = context
        .iter()
        .map(|m| (m.author_name.as_str(), m.text.as_str()))
        .collect();
    assert_eq!(shown, vec![("name 1", "first"), ("name 2", "second")]);
}

#[test]
fn test_blank_messages_are_left_out_but_attachments_stay() {
    let mut picture = message(1, "");
    picture.attachment = Some(MessageAttachment::Image);

    let context = for_model(&[message(5, "  \n"), picture, message(2, "hi")]);

    let names: Vec<&str> = context.iter().map(|m| m.author_name.as_str()).collect();
    assert_eq!(names, vec!["name 1", "name 2"]);
    assert_eq!(context[0].attachment, Some(MessageAttachment::Image));
}
