use super::filter::{
    find_domains, find_in_list, find_outside_list, should_moderate_in_list,
    should_moderate_outside_list,
};

/// Mock domain list returned by `find_domains` (pre-extracted, lowercase).
fn found(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ===========================================================================
// find_domains
// ===========================================================================

/// Table-driven tests for `find_domains`.
///
/// Each entry is `(input_text, expected_domains)`.  An empty expected slice
/// asserts that `find_domains` returns no results at all; otherwise every
/// listed domain must appear somewhere in the output.
#[test]
fn find_domains_table() {
    let cases: &[(&str, &[&str])] = &[
        // --- basic extraction ---
        ("", &[]),
        ("check https://evil.com/path", &["evil.com"]),
        ("visit http://evil.com", &["evil.com"]),
        ("go to www.evil.com/page", &["evil.com"]),
        ("visit evil.com today", &["evil.com"]),
        ("join ttps://sub.example.im", &["sub.example.im"]),
        ("see sub.evil.com for more", &["sub.evil.com"]),
        ("join evil . com to see more", &["evil.com"]),
        ("join sub.evil . com to see more", &["sub.evil.com"]),
        ("join sub .evil . com to see more", &["sub.evil.com"]),
        ("https://evil.com/a/b?x=1&y=2#anchor", &["evil.com"]),
        ("http://evil.com:8080/path", &["evil.com"]),
        ("hello world no links here", &[]),
        ("how to use dot net?", &[]),
        ("http ://verybadsite dot net", &["verybadsite.net"]),
        ("I bought a product.It was great.", &[]),
        ("The store closed.He went home.", &[]),
        ("The movie was great. Watch it again.", &[]),
        ("He arrived. Nobody noticed.", &[]),
        ("hello. AI is too dangerous", &[]),
        ("It was late. Today I rested.", &[]),
        ("Visit us. Online orders welcome.", &[]),
        (
            "http://alpha.com and https://beta.org",
            &["alpha.com", "beta.org"],
        ),
        // --- dot-substitution obfuscation ---
        ("evil[.]com", &["evil.com"]),
        ("evil(.)com", &["evil.com"]),
        ("evil{.}com", &["evil.com"]),
        ("evil dot com", &["evil.com"]),
        ("evil[dot]com", &["evil.com"]),
        ("evil(dot)com", &["evil.com"]),
        ("evil\u{00B7}com", &["evil.com"]), // MIDDLE DOT   U+00B7
        ("evil\u{2022}com", &["evil.com"]), // BULLET        U+2022
        ("evil\u{FF0E}com", &["evil.com"]), // FULLWIDTH .   U+FF0E
        // --- scheme obfuscation ---
        ("hxxp://evil.com/path", &["evil.com"]),
        ("hxxps://evil.com/path", &["evil.com"]),
        ("h**p://evil.com", &["evil.com"]),
        ("h--p://evil.com", &["evil.com"]),
        ("https\u{200B}://evil.com/path", &["evil.com"]), // ZW space stripped
        ("hxxps://evil[.]com/page", &["evil.com"]),       // scheme + dot-sub combined
        // --- space-around-dot obfuscation ---
        ("google. com", &["google.com"]),     // space after dot
        ("google .com", &["google.com"]),     // space before dot
        ("evil  .  com", &["evil.com"]),      // multiple spaces on both sides
        ("https://evil. com", &["evil.com"]), // scheme + space after dot
        ("evil[.] com", &["evil.com"]),       // bracket-dot + trailing space
        ("evil[.]  com", &["evil.com"]),
        ("evil [.] com", &["evil.com"]),
        ("evil (.) com", &["evil.com"]),
        ("evil {.} com", &["evil.com"]),
        ("evil [.]com", &["evil.com"]),
        ("evil (.)com", &["evil.com"]),
        ("evil {.}com", &["evil.com"]),
        // --- additional bracket/delimiter styles for dot-word ---
        ("evil{dot}com", &["evil.com"]),
        ("evil{ dot }com", &["evil.com"]),
        ("evil<dot>com", &["evil.com"]),
        ("evil< dot >com", &["evil.com"]),
        ("evil<.>com", &["evil.com"]),
        // --- language / leet-speak variants ---
        ("evil точка com", &["evil.com"]),
        ("evil[точка]com", &["evil.com"]),
        ("evil d0t com", &["evil.com"]),
        // --- single-character-run (spaced-out) obfuscation ---
        ("H t t p:// a s r i y a n . m e", &["asriyan.me"]),
    ];

    for &(input, must_contain) in cases {
        let got = find_domains(input);
        if must_contain.is_empty() {
            assert!(
                got.is_empty(),
                "input={input:?}: expected no domains, got {got:?}"
            );
        } else {
            for expected in must_contain {
                assert!(
                    got.contains(&expected.to_string()),
                    "input={input:?}: expected domain {expected:?}, got {got:?}",
                );
            }
        }
    }
}

#[test]
fn find_domains_deduplication() {
    // Same domain via scheme URL and bare hostname → deduplicated to one entry.
    let domains = find_domains("https://evil.com is also at evil.com/path");
    assert_eq!(
        domains.iter().filter(|d| *d == "evil.com").count(),
        1,
        "got: {domains:?}",
    );
}

// ===========================================================================
// find_in_list — decision logic (mock domain input)
// ===========================================================================

#[test]
fn in_list_decision_table() {
    // (detected_domains, listed_patterns, expected_result)
    // Domains are already normalised/lowercased (as find_domains returns them).
    let cases: &[(&[&str], &[&str], Option<&str>)] = &[
        (&["evil.com"], &["evil.com"], Some("evil.com")),
        (&["good.com"], &["evil.com"], None),
        (&["sub.evil.com"], &["evil.com"], Some("sub.evil.com")),
        (&["a.b.evil.com"], &["evil.com"], Some("a.b.evil.com")),
        (&["notevil.com"], &["evil.com"], None), // not a suffix match
        (&["evil.com"], &["EVIL.COM"], Some("evil.com")), // case-insensitive pattern
        (&[], &["evil.com"], None),              // no domains found
        (&["evil.com"], &[], None),              // empty list
        (&["good.com", "evil.com"], &["evil.com"], Some("evil.com")),
        (&["evil.com", "bad.org"], &["bad.org"], Some("bad.org")),
        (&["bad.com"], &["sub.bad.com"], None), // subdomain not matched if only parent is listed
    ];

    for &(detected, patterns, expected) in cases {
        let result = find_in_list(&found(detected), &list(patterns));
        assert_eq!(
            result.as_deref(),
            expected,
            "domains={detected:?} list={patterns:?}",
        );
    }
}

/// Smoke test: verifies `find_domains` + `find_in_list` compose correctly.
/// Obfuscation edge-cases are covered by `find_domains_table`.
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

// ===========================================================================
// find_outside_list — decision logic (mock domain input)
// ===========================================================================

#[test]
fn outside_list_decision_table() {
    // (detected_domains, listed_patterns, expected_result)
    let cases: &[(&[&str], &[&str], Option<&str>)] = &[
        (&["good.com"], &["good.com"], None),
        (&["evil.com"], &["good.com"], Some("evil.com")),
        (&["docs.good.com"], &["good.com"], None), // subdomain of listed
        (&[], &["good.com"], None),                // no domains found
        (&["anything.com"], &[], Some("anything.com")), // empty list: every link is outside it
        (&["good.com"], &["GOOD.COM"], None),      // case-insensitive pattern
        (&["good.com", "evil.com"], &["good.com"], Some("evil.com")),
        (&["good.com", "docs.good.com"], &["good.com"], None), // all covered
        (&["good.com"], &["sub.good.com"], Some("good.com")),  // subdomain listed, but not parent
    ];

    for &(detected, patterns, expected) in cases {
        let result = find_outside_list(&found(detected), &list(patterns));
        assert_eq!(
            result.as_deref(),
            expected,
            "domains={detected:?} list={patterns:?}",
        );
    }
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
