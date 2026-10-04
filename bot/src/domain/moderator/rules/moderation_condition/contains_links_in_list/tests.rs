use super::*;

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

#[test]
fn test_rejects_too_long_listed_domain() {
    let err = ContainsLinksInList {
        domains: vec![format!("{}.com", "a".repeat(100))],
    }
    .normalize_and_validate()
    .expect_err("the domain should have been rejected")
    .to_string();
    assert!(err.contains("Domain too long"));
}

#[test]
fn test_domains_are_normalized() {
    let mut condition = ContainsLinksInList {
        domains: vec![
            "b.com".to_string(),
            String::new(),
            "a.com".to_string(),
            "b.com".to_string(),
        ],
    };
    condition.normalize_and_validate().unwrap();
    assert_eq!(condition.domains, vec!["a.com".to_string(), "b.com".to_string()]);
}
