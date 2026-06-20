//! Link-based message filter for the moderator bounded context.
//!
//! Given an incoming message and a list of domain patterns, decides whether
//! the message should be moderated (deleted).
//!
//! # Obfuscation resistance
//!
//! The filter sees through common tricks people use to hide URLs:
//!
//! - **Scheme obfuscation** — `hxxp://`, `hxxps://`, `h**p://`, `h--p://`,
//!   `h__p://`, `ht.tp://`, etc.
//! - **Dot substitution** — `evil[.]com`, `evil(.)com`, `evil{.}com`,
//!   `evil [dot] com`, `evil(dot)com`, middle-dot, bullet, full-width period.
//! - **Zero-width characters** — stripped before matching.
//! - **Case** — domains are compared case-insensitively.
//! - **Subdomains** — blocking / allowing `example.com` also covers
//!   `sub.example.com`, `deep.sub.example.com`, etc.

use regex::Regex;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Compiled regexes
// ---------------------------------------------------------------------------

/// URLs with an explicit http(s):// scheme.
static URL_SCHEME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)https?://[^\\s<>\\[\\](){},;'\"\\\\]+").expect("valid URL_SCHEME_RE")
});

/// www.-prefixed URLs without a scheme.
static URL_WWW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?i)www\\.[^\\s<>\\[\\](){},;'\"\\\\]+").expect("valid URL_WWW_RE")
});

/// Bare hostnames ending in a well-known TLD.
/// We use `\b` at both ends instead of a lookahead (the `regex` crate does
/// not support lookahead assertions).
static BARE_DOMAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b((?:[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?\.)+",
        r"(?:com|org|net|io|ru|ua|de|fr|uk|info|biz|co|app|dev|xyz|me|tv|",
        r"cc|to|sh|ly|gl|link|click|tech|store|shop|news|blog|live|media|",
        r"network|agency|pro|plus|group|team|gov|edu|mil|int|mobi|name|",
        r"coop|aero|tel|cloud|digital|online|site|web|space|world|today|",
        r"top|win|trade|work|vip|club|guru|solutions|company|business|",
        r"services|email|global|chat|social|ai|ygg))\b",
    ))
    .expect("valid BARE_DOMAIN_RE")
});

/// Scheme-obfuscation patterns: `hxxp://`, `h**p://`, `h--ps://`, etc.
static SCHEME_OBFUSC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)h[xt*_\-\.]{2,4}ps?://").expect("valid SCHEME_OBFUSC_RE"));

/// Sequences of single characters separated by spaces, e.g. `h t t p` or
/// `e v i l`.  Each character must sit at a word boundary so that normal
/// words like `"am"` are not split.
static SINGLE_CHAR_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-z0-9](?: [a-z0-9])+\b").expect("valid SINGLE_CHAR_RUN_RE")
});

/// Spaces that appear immediately after a `://` separator.
static URL_SCHEME_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"://\s+").expect("valid URL_SCHEME_SPACE_RE"));

// ---------------------------------------------------------------------------
// Normalisation
// ---------------------------------------------------------------------------

fn normalize_obfuscation(text: &str) -> String {
    let mut s = text.to_lowercase();

    // Dot substitutions — longer patterns first to avoid partial matches
    for pat in &[
        "[dot]", "(dot)", " dot ", "[ dot ]", "( dot )", "[.]", "(.)", "{.}",
        " . ", // space-dot-space
    ] {
        s = s.replace(pat, ".");
    }
    s = s.replace('\u{00B7}', "."); // MIDDLE DOT  ·
    s = s.replace('\u{2022}', "."); // BULLET       •
    s = s.replace('\u{FF0E}', "."); // FULLWIDTH .  ．
    s = s.replace('\u{30FB}', "."); // KATAKANA ·   ・

    // Strip zero-width chars (invisible separators)
    s = s.replace('\u{200B}', "");
    s = s.replace('\u{200C}', "");
    s = s.replace('\u{200D}', "");
    s = s.replace('\u{FEFF}', "");

    // Slash substitution
    s = s.replace("[/]", "/");

    // Scheme obfuscation
    s = s.replace("hxxps://", "https://");
    s = s.replace("hxxp://", "http://");
    s = SCHEME_OBFUSC_RE.replace_all(&s, "https://").into_owned();

    // Collapse runs of single chars separated by spaces: "h t t p" → "http",
    // "a s r" → "asr".  Each char must sit at a word boundary so normal
    // multi-char words are not affected.
    s = SINGLE_CHAR_RUN_RE
        .replace_all(&s, |caps: &regex::Captures| caps[0].replace(' ', ""))
        .into_owned();

    // Strip any spaces that immediately follow the scheme separator.
    s = URL_SCHEME_SPACE_RE.replace_all(&s, "://").into_owned();

    // Collapse remaining spaces inside the host portion of URLs.
    // A space is collapsed only when the immediately following token (up to
    // the next whitespace) contains a dot — this indicates a domain fragment
    // rather than a regular word boundary.
    s = collapse_url_host_spaces(&s);

    s
}

/// Collapse spaces inside the *host* part of URLs (after `://`, before the
/// first `/`/`?`/`#`).  A space is only collapsed when the next token
/// contains a `.`, which is a reliable signal that it is a domain fragment
/// rather than ordinary following text.
fn collapse_url_host_spaces(text: &str) -> String {
    if !text.contains("://") {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut result = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 2 < n && chars[i] == ':' && chars[i + 1] == '/' && chars[i + 2] == '/' {
            result.push_str("://");
            i += 3;
            // Consume the host portion, collapsing intra-host spaces.
            loop {
                if i >= n {
                    break;
                }
                let c = chars[i];
                match c {
                    '/' | '?' | '#' => {
                        // Start of path — append the rest of the URL verbatim.
                        while i < n
                            && !chars[i].is_whitespace()
                            && !"<>[]{},;'\"\\".contains(chars[i])
                        {
                            result.push(chars[i]);
                            i += 1;
                        }
                        break;
                    }
                    ' ' | '\t' => {
                        // Skip whitespace.
                        let space_end = {
                            let mut j = i;
                            while j < n && (chars[j] == ' ' || chars[j] == '\t') {
                                j += 1;
                            }
                            j
                        };
                        // Collect the next token (up to next whitespace or
                        // URL structural delimiter).
                        let token_end = {
                            let mut j = space_end;
                            while j < n
                                && chars[j] != ' '
                                && chars[j] != '\t'
                                && chars[j] != '/'
                                && chars[j] != '?'
                                && chars[j] != '#'
                                && (chars[j].is_alphanumeric() || ".-_~%".contains(chars[j]))
                            {
                                j += 1;
                            }
                            j
                        };
                        let next_token: String = chars[space_end..token_end].iter().collect();
                        if next_token.contains('.') {
                            // Looks like a domain fragment — collapse the space.
                            i = space_end;
                        } else {
                            // Ordinary word boundary — stop the URL here.
                            break;
                        }
                    }
                    _ if c.is_alphanumeric() || ".-_~%:@".contains(c) => {
                        result.push(c);
                        i += 1;
                    }
                    _ => break,
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Domain extraction
// ---------------------------------------------------------------------------

fn extract_domain(raw: &str) -> Option<String> {
    let s = raw
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let host = s
        .split(|c: char| c == '/' || c == '?' || c == '#' || c == ':')
        .next()
        .unwrap_or(s);
    let host = host.trim_matches('.');
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.to_lowercase())
}

/// Extract all link domains from `text`, applying obfuscation normalisation
/// first.  Domains are returned lower-case and deduplicated.
pub fn find_domains(text: &str) -> Vec<String> {
    let normalized = normalize_obfuscation(text);
    let mut domains: Vec<String> = Vec::new();

    for m in URL_SCHEME_RE.find_iter(&normalized) {
        if let Some(d) = extract_domain(m.as_str()) {
            if !domains.contains(&d) {
                domains.push(d);
            }
        }
    }
    for m in URL_WWW_RE.find_iter(&normalized) {
        let with_scheme = format!("http://{}", m.as_str());
        if let Some(d) = extract_domain(&with_scheme) {
            if !domains.contains(&d) {
                domains.push(d);
            }
        }
    }
    for caps in BARE_DOMAIN_RE.captures_iter(&normalized) {
        if let Some(m) = caps.get(1) {
            // Route through extract_domain so that www. is stripped and
            // we don't end up with both "evil.com" and "www.evil.com".
            if let Some(d) = extract_domain(m.as_str()) {
                if !domains.contains(&d) {
                    domains.push(d);
                }
            }
        }
    }

    domains
}

// ---------------------------------------------------------------------------
// Domain-pattern matching
// ---------------------------------------------------------------------------

/// `"sub.evil.com"` matches pattern `"evil.com"` (subdomain), exact match
/// also returns `true`.
fn domain_matches(domain: &str, pattern: &str) -> bool {
    let d = domain.to_lowercase();
    let p = pattern.to_lowercase();
    d == p || d.ends_with(&format!(".{p}"))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `Some(domain)` if `text` contains a link whose domain is on the
/// blacklist, or `None` if all links are allowed (or there are no links).
pub fn should_moderate_blacklist(text: &str, blocked: &[String]) -> Option<String> {
    let domains = find_domains(text);
    for domain in &domains {
        for b in blocked {
            if domain_matches(domain, b) {
                return Some(domain.clone());
            }
        }
    }
    None
}

/// Returns `Some(domain)` if `text` contains a link whose domain is **not**
/// covered by the allowlist, or `None` if every link is allowed (or there are
/// no links at all).
pub fn should_moderate_whitelist(text: &str, allowed: &[String]) -> Option<String> {
    let domains = find_domains(text);
    for domain in &domains {
        let is_allowed = allowed.iter().any(|a| domain_matches(domain, a));
        if !is_allowed {
            return Some(domain.clone());
        }
    }
    None
}

use super::top100;

/// Returns `Some(domain)` if `text` contains a link whose domain is **not**
/// in the built-in top-100 preset allowlist.  Messages with no links are
/// always allowed.
pub fn should_moderate_whitelist_top100(text: &str) -> Option<String> {
    let domains = find_domains(text);
    for domain in &domains {
        let is_allowed = top100::DOMAINS.iter().any(|a| domain_matches(domain, a));
        if !is_allowed {
            return Some(domain.clone());
        }
    }
    None
}
