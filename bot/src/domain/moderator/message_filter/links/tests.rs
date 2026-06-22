use super::filter::{find_domains, should_moderate_blacklist, should_moderate_whitelist};

fn blocked(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
fn allowed(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ===========================================================================
// find_domains — basic extraction
// ===========================================================================

#[test]
fn extract_https_url() {
    assert_eq!(find_domains("check https://evil.com/path"), vec!["evil.com"]);
}

#[test]
fn extract_http_url() {
    assert_eq!(find_domains("visit http://evil.com"), vec!["evil.com"]);
}

#[test]
fn extract_www_url() {
    assert_eq!(find_domains("go to www.evil.com/page"), vec!["evil.com"]);
}

#[test]
fn extract_bare_domain() {
    let domains = find_domains("visit evil.com today");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn extract_subdomain() {
    assert_eq!(
        find_domains("see sub.evil.com for more"),
        vec!["sub.evil.com"]
    );
}

#[test]
fn extract_domain_with_path_and_query() {
    assert_eq!(
        find_domains("https://evil.com/a/b?x=1&y=2#anchor"),
        vec!["evil.com"]
    );
}

#[test]
fn extract_domain_with_port() {
    assert_eq!(
        find_domains("http://evil.com:8080/path"),
        vec!["evil.com"]
    );
}

#[test]
fn no_domain_in_plain_text() {
    assert!(find_domains("hello world no links here").is_empty());
}

#[test]
fn multiple_domains_extracted() {
    let domains = find_domains("http://alpha.com and https://beta.org are both present");
    assert!(domains.contains(&"alpha.com".to_string()));
    assert!(domains.contains(&"beta.org".to_string()));
}

// ===========================================================================
// find_domains — obfuscation normalisation
// ===========================================================================

#[test]
fn obfusc_square_bracket_dot() {
    let domains = find_domains("evil[.]com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_parenthesis_dot() {
    let domains = find_domains("evil(.)com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_curly_dot() {
    let domains = find_domains("evil{.}com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_dot_word() {
    let domains = find_domains("evil dot com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_dot_word_bracketed() {
    let domains = find_domains("evil[dot]com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_dot_word_parens() {
    let domains = find_domains("evil(dot)com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_middle_dot_unicode() {
    // U+00B7 MIDDLE DOT ·
    let domains = find_domains("evil\u{00B7}com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_bullet_unicode() {
    // U+2022 BULLET •
    let domains = find_domains("evil\u{2022}com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_fullwidth_period_unicode() {
    // U+FF0E FULLWIDTH FULL STOP ．
    let domains = find_domains("evil\u{FF0E}com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_hxxp_scheme() {
    let domains = find_domains("hxxp://evil.com/path");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_hxxps_scheme() {
    let domains = find_domains("hxxps://evil.com/path");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_h_star_star_p_scheme() {
    let domains = find_domains("h**p://evil.com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_h_dash_dash_p_scheme() {
    let domains = find_domains("h--p://evil.com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_zero_width_chars_in_scheme() {
    // Zero-width spaces can be inserted to break pattern matching
    // "https\u{200B}://evil.com" → after stripping ZW chars → "https://evil.com"
    let domains = find_domains("https\u{200B}://evil.com/path");
    // After ZW removal + SCHEME_RE this should resolve; included for coverage
    // (exact behaviour depends on where ZW char lands in the scheme vs host)
    let _ = domains; // not asserting exact result, just no panic
}

#[test]
fn obfusc_combined_dot_and_scheme() {
    let domains = find_domains("hxxps://evil[.]com/page");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_space_after_dot() {
    // "google. com" — space inserted between the dot and the TLD
    let domains = find_domains("google. com");
    assert!(domains.contains(&"google.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_space_before_dot() {
    // "google .com" — space inserted before the dot
    let domains = find_domains("google .com");
    assert!(domains.contains(&"google.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_multiple_spaces_around_dot() {
    // "evil  .  com" — multiple spaces on both sides of the dot
    let domains = find_domains("evil  .  com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_space_after_dot_with_scheme() {
    // "https://evil. com" — space after dot in a schemed URL
    let domains = find_domains("https://evil. com");
    assert!(domains.contains(&"evil.com".to_string()), "got: {domains:?}");
}

#[test]
fn obfusc_spaced_chars_scheme_and_host() {
    // "H t t p:// a s r tiyan . ru" — characters of the scheme and parts of
    // the host are space-separated, and the dot is surrounded by spaces.
    let domains = find_domains("H t t p:// a s r tiyan . ru");
    assert!(
        domains.contains(&"asrtiyan.ru".to_string()),
        "got: {domains:?}"
    );
}

#[test]
fn blacklist_spaced_chars_is_moderated() {
    assert!(
        should_moderate_blacklist(
            "H t t p:// a s r tiyan . ru",
            &blocked(&["asrtiyan.ru"])
        )
        .is_some()
    );
}

#[test]
fn whitelist_spaced_chars_non_allowed_is_moderated() {
    assert!(
        should_moderate_whitelist(
            "H t t p:// a s r tiyan . ru",
            &blocked(&["github.com"])
        )
        .is_some()
    );
}

#[test]
fn deduplication_across_extraction_modes() {
    // Same domain appears via scheme URL and bare domain → only once
    let domains = find_domains("https://evil.com is also at evil.com/path");
    assert_eq!(domains.iter().filter(|d| *d == "evil.com").count(), 1);
}

// ===========================================================================
// should_moderate_blacklist
// ===========================================================================

#[test]
fn blacklist_blocked_domain_is_moderated() {
    assert!(
        should_moderate_blacklist("visit https://evil.com now", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_unblocked_domain_is_allowed() {
    assert!(should_moderate_blacklist(
        "visit https://good.com",
        &blocked(&["evil.com"])
    )
    .is_none());
}

#[test]
fn blacklist_returns_matched_domain() {
    let result = should_moderate_blacklist("https://evil.com", &blocked(&["evil.com"]));
    assert_eq!(result.as_deref(), Some("evil.com"));
}

#[test]
fn blacklist_subdomain_of_blocked_is_moderated() {
    assert!(
        should_moderate_blacklist("https://sub.evil.com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_deep_subdomain_of_blocked_is_moderated() {
    assert!(
        should_moderate_blacklist("https://a.b.evil.com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_pattern_is_not_suffix_matched_on_different_domain() {
    // "notevil.com" should NOT match pattern "evil.com"
    assert!(
        should_moderate_blacklist("https://notevil.com", &blocked(&["evil.com"])).is_none()
    );
}

#[test]
fn blacklist_case_insensitive_domain() {
    assert!(
        should_moderate_blacklist("https://EVIL.COM/path", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_case_insensitive_pattern() {
    assert!(
        should_moderate_blacklist("https://evil.com", &blocked(&["EVIL.COM"])).is_some()
    );
}

#[test]
fn blacklist_no_links_in_message_is_not_moderated() {
    assert!(
        should_moderate_blacklist("just plain text here", &blocked(&["evil.com"])).is_none()
    );
}

#[test]
fn blacklist_empty_blocked_list_is_not_moderated() {
    assert!(
        should_moderate_blacklist("https://evil.com", &[]).is_none()
    );
}

#[test]
fn blacklist_obfusc_bracket_dot() {
    assert!(
        should_moderate_blacklist("evil[.]com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_dot_word() {
    assert!(
        should_moderate_blacklist("evil dot com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_space_after_dot() {
    // "google. com" — a space is inserted after the dot to break URL detection
    assert!(
        should_moderate_blacklist("google. com", &blocked(&["google.com"])).is_some()
    );
    assert!(
        should_moderate_blacklist("evil[.] com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_space_before_dot() {
    // "google .com" — space before the dot
    assert!(
        should_moderate_blacklist("google .com", &blocked(&["google.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_multiple_spaces_around_dot() {
    // "google  .  com" — multiple spaces on both sides of the dot
    assert!(
        should_moderate_blacklist("google  .  com", &blocked(&["google.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_scheme_spaced_dot() {
    // "https://evil. com" — space after dot in a schemed URL
    assert!(
        should_moderate_blacklist("https://evil. com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_hxxp_scheme() {
    assert!(
        should_moderate_blacklist("hxxp://evil.com/path", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_obfusc_combined() {
    assert!(
        should_moderate_blacklist("hxxps://evil[.]com", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_www_prefix() {
    assert!(
        should_moderate_blacklist("www.evil.com/page", &blocked(&["evil.com"])).is_some()
    );
}

#[test]
fn blacklist_first_blocked_domain_returned_from_multiple() {
    let result = should_moderate_blacklist(
        "go to https://evil.com and http://bad.org",
        &blocked(&["bad.org", "evil.com"]),
    );
    assert!(result.is_some());
}

// ===========================================================================
// should_moderate_whitelist
// ===========================================================================

#[test]
fn whitelist_allowed_domain_is_not_moderated() {
    assert!(
        should_moderate_whitelist("https://good.com/page", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_disallowed_domain_is_moderated() {
    assert!(
        should_moderate_whitelist("https://evil.com", &allowed(&["good.com"])).is_some()
    );
}

#[test]
fn whitelist_returns_offending_domain() {
    let result = should_moderate_whitelist("https://evil.com", &allowed(&["good.com"]));
    assert_eq!(result.as_deref(), Some("evil.com"));
}

#[test]
fn whitelist_subdomain_of_allowed_is_not_moderated() {
    assert!(
        should_moderate_whitelist("https://docs.good.com", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_no_links_is_not_moderated() {
    assert!(
        should_moderate_whitelist("just plain text", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_empty_allowed_list_blocks_all_links() {
    assert!(
        should_moderate_whitelist("https://anything.com", &[]).is_some()
    );
}

#[test]
fn whitelist_case_insensitive_domain() {
    assert!(
        should_moderate_whitelist("https://GOOD.COM", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_case_insensitive_pattern() {
    assert!(
        should_moderate_whitelist("https://good.com", &allowed(&["GOOD.COM"])).is_none()
    );
}

#[test]
fn whitelist_obfusc_bracket_dot() {
    // evil[.]com obfuscated — must be detected and blocked (not in whitelist)
    assert!(
        should_moderate_whitelist("evil[.]com", &allowed(&["good.com"])).is_some()
    );
}

#[test]
fn whitelist_obfusc_allowed_domain_bracket_dot() {
    // good[.]com is the allowed domain written with obfuscation
    assert!(
        should_moderate_whitelist("good[.]com/path", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_obfusc_hxxp_disallowed() {
    assert!(
        should_moderate_whitelist("hxxp://evil.com", &allowed(&["good.com"])).is_some()
    );
}

#[test]
fn whitelist_multiple_links_one_disallowed() {
    // good.com is fine, but evil.com is not → moderate
    assert!(
        should_moderate_whitelist(
            "https://good.com and https://evil.com",
            &allowed(&["good.com"])
        )
        .is_some()
    );
}

#[test]
fn whitelist_multiple_links_all_allowed() {
    assert!(
        should_moderate_whitelist(
            "https://good.com and https://docs.good.com",
            &allowed(&["good.com"])
        )
        .is_none()
    );
}

#[test]
fn whitelist_www_prefix_allowed() {
    assert!(
        should_moderate_whitelist("www.good.com/page", &allowed(&["good.com"])).is_none()
    );
}

#[test]
fn whitelist_www_prefix_disallowed() {
    assert!(
        should_moderate_whitelist("www.evil.com/page", &allowed(&["good.com"])).is_some()
    );
}
