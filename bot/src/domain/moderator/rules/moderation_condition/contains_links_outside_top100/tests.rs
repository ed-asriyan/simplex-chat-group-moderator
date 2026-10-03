use super::*;

#[test]
fn test_top_100_and_the_owners_list_both_cover_a_link() {
    let none: Vec<String> = Vec::new();
    assert!(should_moderate_outside_top100("https://github.com/rust-lang", &none).is_none());
    assert!(should_moderate_outside_top100("https://evil.example", &none).is_some());
    assert!(
        should_moderate_outside_top100("https://evil.example", &["evil.example".to_string()])
            .is_none()
    );
    assert!(should_moderate_outside_top100("no links here", &none).is_none());
}

#[test]
fn test_domains_are_normalized() {
    let mut condition = ContainsLinksOutsideTop100 {
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
