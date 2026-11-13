use super::*;

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// US-E31: a list of up to 10000 entries of up to 100 characters each.
#[test]
fn test_rejects_too_many_words() {
    let err = MemberNameContainsWords {
        keywords: (0..10_001).map(|i| format!("word{i}")).collect(),
    }
    .normalize_and_validate()
    .expect_err("the list should have been rejected")
    .to_string();
    assert!(err.contains("Too many keywords"), "{err}");
}

// US-E31
#[test]
fn test_a_word_of_100_characters_is_accepted_and_one_of_101_is_not() {
    let mut longest = MemberNameContainsWords {
        keywords: vec!["a".repeat(100)],
    };
    longest.normalize_and_validate().unwrap();
    let err = MemberNameContainsWords {
        keywords: vec!["a".repeat(101)],
    }
    .normalize_and_validate()
    .unwrap_err()
    .to_string();
    assert!(err.contains("Keyword"), "{err}");
}

// US-E31: blank entries are dropped and duplicates folded, as for a message's words.
#[test]
fn test_words_are_normalized() {
    let mut condition = MemberNameContainsWords {
        keywords: list(&["b", "", "a", "b"]),
    };
    condition.normalize_and_validate().unwrap();
    assert_eq!(condition.keywords, list(&["a", "b"]));
}

// US-E27–US-E29: what a `Not` around it says when it matches.
#[test]
fn test_it_describes_what_it_looks_for() {
    let condition = MemberNameContainsWords {
        keywords: list(&["casino"]),
    };
    assert_eq!(condition.describe(), "name contains one of the listed words");
}
