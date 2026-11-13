use super::*;

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// US-E32: a list of up to 10000 domains of up to 100 characters each.
#[test]
fn test_rejects_too_many_domains() {
    let err = MemberNameContainsLinksOutsideList {
        domains: (0..10_001).map(|i| format!("site{i}.com")).collect(),
    }
    .normalize_and_validate()
    .expect_err("the list should have been rejected")
    .to_string();
    assert!(err.contains("Too many domains"), "{err}");
}

// US-E32
#[test]
fn test_a_domain_of_101_characters_is_rejected() {
    let err = MemberNameContainsLinksOutsideList {
        domains: vec!["a".repeat(101)],
    }
    .normalize_and_validate()
    .unwrap_err()
    .to_string();
    assert!(err.contains("Domain"), "{err}");
}

// US-E32: an empty list is valid, and matches every link.
#[test]
fn test_an_empty_list_is_valid() {
    let mut condition = MemberNameContainsLinksOutsideList { domains: vec![] };
    condition.normalize_and_validate().unwrap();
    assert!(condition.domains.is_empty());
}

// US-E32
#[test]
fn test_domains_are_normalized() {
    let mut condition = MemberNameContainsLinksOutsideList {
        domains: list(&["b.com", "", "a.com", "b.com"]),
    };
    condition.normalize_and_validate().unwrap();
    assert_eq!(condition.domains, list(&["a.com", "b.com"]));
}

// US-E27–US-E29
#[test]
fn test_it_describes_what_it_looks_for() {
    let condition = MemberNameContainsLinksOutsideList { domains: vec![] };
    assert_eq!(
        condition.describe(),
        "name links to a website outside the list"
    );
}
