//! The isolation of conditions and actions, checked on the source.
//!
//! Every condition and every action is a module of its own, and the point of
//! that is that adding, changing or removing one touches its module and one
//! line of its registry, nothing else. Rust's visibility rules cannot say so:
//! a sibling module is always visible. So this test reads the moderator's
//! sources and fails on anything that ties a condition or an action to the
//! rest:
//! - a leaf is named only by its own module and its registry line — not by
//!   another leaf, the engine, `common`, the application or the ports;
//! - a leaf reaches only its registry, the ports and `common`;
//! - `common` serves the leaves without knowing the engine;
//! - outside `rules`, nothing names a variant of either enum.
//!
//! The leaves are read from the `conditions!` and `actions!` invocations, so a
//! new one is checked without touching this file. Test files are exempt: a
//! test may build whatever it needs.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Each registry: its macro, and the module (a file and a directory under
/// `rules`) that invokes it and holds its leaves.
const REGISTRIES: &[(&str, &str)] = &[
    ("conditions", "moderation_condition"),
    ("actions", "moderation_action"),
];

/// Where a leaf may reach with a `crate::` path.
const LEAF_MAY_USE: &[&str] = &[
    "crate::domain::moderator::ports",
    "crate::domain::moderator::rules::common",
];

struct Leaf {
    variant: String,
    module: String,
    /// The registry's directory, where the leaf's file and directory live.
    dir: PathBuf,
    /// The leaf's line in the registry invocation.
    registry_line: String,
}

impl Leaf {
    fn owns(&self, path: &Path) -> bool {
        path == self.dir.join(format!("{}.rs", self.module))
            || path.starts_with(self.dir.join(&self.module))
    }
}

/// The leaves a registry declares, one `Variant => module,` line each.
fn leaves_of(rules: &Path, macro_name: &str, module: &str) -> Vec<Leaf> {
    let source = fs::read_to_string(rules.join(format!("{module}.rs"))).unwrap();
    let entry = Regex::new(r"^\s*(\w+)\s*=>\s*(\w+),\s*$").unwrap();
    let invocation = source
        .split_once(&format!("\n{macro_name}! {{\n"))
        .unwrap_or_else(|| panic!("no `{macro_name}!` invocation in {module}.rs"))
        .1;
    let body = invocation.split_once("\n}").unwrap().0;
    body.lines()
        .filter_map(|line| entry.captures(line))
        .map(|captures| Leaf {
            variant: captures[1].to_string(),
            module: captures[2].to_string(),
            dir: rules.join(module),
            registry_line: captures[0].trim().to_string(),
        })
        .collect()
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
    name.is_some_and(|name| name.ends_with("tests.rs") || name == "fakes.rs")
}

/// The lines of a file without their comments: a comment may name anything.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let code = line.split("//").next().unwrap_or_default();
            (index + 1, code.to_string())
        })
        .collect()
}

#[test]
fn test_conditions_and_actions_are_isolated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let moderator = root.join("src/domain/moderator");
    let rules = moderator.join("rules");

    let leaves: Vec<Leaf> = REGISTRIES
        .iter()
        .flat_map(|(macro_name, module)| {
            let leaves = leaves_of(&rules, macro_name, module);
            // A misread registry would otherwise pass with nothing checked.
            assert!(!leaves.is_empty(), "no leaves read from `{macro_name}!`");
            leaves
        })
        .collect();
    for leaf in &leaves {
        assert!(
            leaf.dir.join(format!("{}.rs", leaf.module)).is_file(),
            "`{}` has no module file",
            leaf.registry_line
        );
    }

    let mut files = Vec::new();
    rust_files(&moderator, &mut files);
    files.retain(|path| !is_test_file(path));
    files.sort();

    let names: Vec<(&Leaf, Regex)> = leaves
        .iter()
        .map(|leaf| {
            let pattern = format!(r"\b{}\b|\b{}::", leaf.variant, leaf.module);
            (leaf, Regex::new(&pattern).unwrap())
        })
        .collect();
    let crate_path = Regex::new(r"\bcrate(::\w+)+").unwrap();
    let escapes_leaf = Regex::new(r"\bsuper::super\b").unwrap();
    let common_reaches_engine =
        Regex::new(r"\bsuper::super\b|\brules::(\w+)|\bModeration(Condition|Action|Rule)\b")
            .unwrap();
    let names_variant = Regex::new(r"\bModeration(Condition|Action)::\w+").unwrap();

    let mut violations = Vec::new();
    for path in &files {
        let shown = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        let owner = leaves.iter().find(|leaf| leaf.owns(path));
        let in_common = path.starts_with(rules.join("common")) || path == &rules.join("common.rs");
        let in_rules = path.starts_with(&rules) || path == &rules.with_extension("rs");

        for (number, code) in code_lines(path) {
            let mut violation = |reason: String| {
                violations.push(format!("{shown}:{number}: {reason}\n    {}", code.trim()));
            };

            for (leaf, name) in &names {
                let own = owner.is_some_and(|owner| owner.variant == leaf.variant);
                let registry_line = code.trim() == leaf.registry_line;
                if !own && !registry_line && name.is_match(&code) {
                    violation(format!(
                        "`{}` is named outside its own module and its registry line",
                        leaf.variant
                    ));
                }
            }

            if owner.is_some() {
                for path in crate_path.find_iter(&code) {
                    if !LEAF_MAY_USE
                        .iter()
                        .any(|allowed| path.as_str().starts_with(allowed))
                    {
                        violation(format!(
                            "a leaf reaches only its registry, the ports and `common`, not `{}`",
                            path.as_str()
                        ));
                    }
                }
                if escapes_leaf.is_match(&code) {
                    violation("a leaf reaches its registry as `super`, nothing above it".into());
                }
            }

            if in_common
                && let Some(found) = common_reaches_engine.captures(&code)
                && found
                    .get(1)
                    .is_none_or(|module| module.as_str() != "common")
            {
                violation("`common` serves the leaves and knows nothing of the engine".into());
            }

            if !in_rules && names_variant.is_match(&code) {
                violation(
                    "outside `rules`, conditions and actions are asked, never matched on".into(),
                );
            }
        }
    }

    assert!(
        violations.is_empty(),
        "conditions and actions are not isolated:\n{}",
        violations.join("\n")
    );
}
