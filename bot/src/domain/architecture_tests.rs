//! The domain's boundary, checked on the source.
//!
//! Rust's visibility rules cannot say "the domain never reaches outside": every
//! public module of the crate is visible from every other one. So this test
//! reads the domain's sources and fails on anything that crosses the line —
//! infrastructure, the other bounded context, a crate that is not on the list
//! below, I/O, or the clock. Test files are exempt: they may build fakes from
//! in-memory adapters and run on a real runtime.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// External crates the domain may use: pure computation, no I/O. A crate joins
/// this list on purpose, never by accident — which is the point of the list.
const ALLOWED_CRATES: &[&str] = &[
    "async_trait",
    "chrono",
    "futures",
    "icu_normalizer",
    "icu_properties",
    "lz_str",
    "regex",
    "serde",
];

/// The bounded contexts, each a directory (and a file) under `src/domain`.
/// They talk only through the routers in `infrastructure/adapters`.
const CONTEXTS: &[&str] = &["bot_dm", "moderator"];

/// What else the domain must not touch, beyond infrastructure and other
/// crates: the outside world, and the clock — "now" is always handed in.
const FORBIDDEN_CALLS: &[(&str, &str)] = &[
    (
        r"\bstd::(fs|io|net|process|env|thread)\b",
        "I/O belongs in infrastructure",
    ),
    (
        r"\b(Utc|Local|SystemTime|Instant)::now\b",
        "the domain is handed the time, it never reads a clock",
    ),
];

struct Rule {
    pattern: Regex,
    reason: String,
}

fn rule(pattern: &str, reason: impl Into<String>) -> Rule {
    Rule {
        pattern: Regex::new(pattern).unwrap(),
        reason: reason.into(),
    }
}

/// The crate's dependency names as code spells them (`-` becomes `_`).
fn dependencies(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if in_dependencies && let Some((name, _)) = line.split_once('=') {
            names.push(name.trim().replace('-', "_"));
        }
    }
    names
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str());
    name.is_some_and(|name| name.ends_with("tests.rs"))
        || path.components().any(|part| part.as_os_str() == "tests")
}

/// The bounded context a file belongs to: `src/domain/moderator.rs` and
/// everything under `src/domain/moderator/` are `moderator`.
fn context_of<'a>(domain: &Path, path: &'a Path) -> Option<&'a str> {
    let first = path.strip_prefix(domain).ok()?.components().next()?;
    let name = first.as_os_str().to_str()?;
    Some(name.strip_suffix(".rs").unwrap_or(name))
}

#[test]
fn test_domain_reaches_nothing_outside_itself() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let domain = root.join("src/domain");

    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let mut rules: Vec<Rule> = dependencies(&manifest)
        .into_iter()
        .filter(|name| !ALLOWED_CRATES.contains(&name.as_str()))
        .map(|name| {
            rule(
                &format!(r"\b{name}(::|!)|\buse {name}\b"),
                format!("`{name}` is not on the domain's list of crates"),
            )
        })
        .collect();
    rules.push(rule(
        r"\binfrastructure::",
        "the domain never depends on infrastructure",
    ));
    rules.extend(
        FORBIDDEN_CALLS
            .iter()
            .map(|(pattern, reason)| rule(pattern, *reason)),
    );

    let mut files = Vec::new();
    rust_files(&domain, &mut files);
    files.retain(|path| !is_test_file(path));
    files.sort();
    // A wrong path would otherwise pass with nothing checked.
    for context in CONTEXTS {
        assert!(
            files
                .iter()
                .any(|path| context_of(&domain, path) == Some(context)),
            "no source found for the `{context}` context under {}",
            domain.display()
        );
    }

    let mut violations = Vec::new();
    for path in &files {
        let context = context_of(&domain, path);
        let other_contexts: Vec<Rule> = CONTEXTS
            .iter()
            .filter(|other| Some(**other) != context)
            .map(|other| {
                rule(
                    &format!(r"\b{other}::"),
                    format!("the `{other}` context is reached only through a router"),
                )
            })
            .collect();

        let source = fs::read_to_string(path).unwrap();
        for (index, line) in source.lines().enumerate() {
            // Comments may name what the code must not use.
            let code = line.split("//").next().unwrap_or_default();
            for rule in rules.iter().chain(&other_contexts) {
                if rule.pattern.is_match(code) {
                    violations.push(format!(
                        "{}:{}: {}\n    {}",
                        path.strip_prefix(root).unwrap_or(path).display(),
                        index + 1,
                        rule.reason,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the domain reaches outside itself:\n{}",
        violations.join("\n")
    );
}
