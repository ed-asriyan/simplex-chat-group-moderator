pub fn should_moderate(message: &str, messages: &[String], case_sensitive: bool) -> Option<String> {
    if case_sensitive {
        for candidate in messages {
            if message == candidate {
                return Some(candidate.to_string());
            }
        }
    } else {
        for candidate in messages {
            if message.eq_ignore_ascii_case(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}
