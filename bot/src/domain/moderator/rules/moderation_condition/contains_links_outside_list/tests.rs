use super::*;

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
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

#[test]
fn test_rejects_too_many_domains() {
    let err = ContainsLinksOutsideList {
        domains: (0..10_001).map(|i| format!("site{i}.com")).collect(),
    }
    .normalize_and_validate()
    .expect_err("the list should have been rejected")
    .to_string();
    assert!(err.contains("Too many domains"));
}

#[test]
fn test_domains_are_normalized() {
    let mut condition = ContainsLinksOutsideList {
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
