use super::{should_moderate_in_list, should_moderate_outside_list};

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Smoke test: verifies `find_domains` + `find_in_list` compose correctly.
/// Obfuscation edge-cases are covered by `find_domains_table` in
/// `common::domains`.
#[test]
fn in_list_pipeline_smoke() {
    assert!(should_moderate_in_list("https://evil.com", &list(&["evil.com"])).is_some());
    assert!(should_moderate_in_list("just plain text", &list(&["evil.com"])).is_none());
}

/// Integration test: full pipeline with obfuscated input.
#[test]
fn in_list_spaced_chars_is_moderated() {
    assert!(
        should_moderate_in_list("H t t p:// a s r tiyan . ru", &list(&["asrtiyan.ru"])).is_some()
    );
}

/// Smoke test: verifies `find_domains` + `find_outside_list` compose correctly.
#[test]
fn outside_list_pipeline_smoke() {
    assert!(should_moderate_outside_list("https://good.com", &list(&["good.com"])).is_none());
    assert!(should_moderate_outside_list("https://evil.com", &list(&["good.com"])).is_some());
}

/// Integration test: full pipeline with obfuscated input.
#[test]
fn outside_list_spaced_chars_unlisted_is_moderated() {
    assert!(
        should_moderate_outside_list("H t t p:// a s r tiyan . ru", &list(&["github.com"]))
            .is_some()
    );
}
