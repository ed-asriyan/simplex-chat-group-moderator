pub fn should_moderate(message: &str, blocked: &[String], case_sensitive: bool) -> Option<String> {
    if case_sensitive {
        for blocked in blocked {
            if message == blocked {
                return Some(blocked.to_string());
            }
        }
    } else {
        for blocked in blocked {
            if message.eq_ignore_ascii_case(blocked) {
                return Some(blocked.to_string());
            }
        }
    }
    None
}
