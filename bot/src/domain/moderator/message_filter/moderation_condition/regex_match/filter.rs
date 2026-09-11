use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// How many compiled patterns to keep. A group's patterns are capped per rule (100 each),
/// so this holds 1000+ groups (100k ÷ 100 = 1000 groups). Overflow on reaching this
/// limit clears everything and starts fresh — extremely rare, and memory cost (~10–50 MB)
/// is negligible.
const MAX_CACHED_PATTERNS: usize = 100_000;

/// Compiling a pattern costs ~70 µs while matching it costs a fraction of that, and the
/// same handful of patterns is used for every message a group receives — so compilation
/// is memoized process-wide.
///
/// This is a cache, not state: it never changes what [`should_moderate`] returns for a
/// given input, so the module stays a pure predicate from the caller's point of view.
static COMPILED: LazyLock<Mutex<HashMap<String, Arc<Regex>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The compiled form of `pattern`, or `None` if it does not compile.
fn compiled(pattern: &str) -> Option<Arc<Regex>> {
    let mut cache = COMPILED.lock().expect("regex cache poisoned");
    if let Some(regex) = cache.get(pattern) {
        // An `Arc` clone, so the compiled program and its internal match-cache pool are
        // shared rather than rebuilt; the lock is released before matching.
        return Some(Arc::clone(regex));
    }

    let regex = Arc::new(Regex::new(pattern).ok()?);
    if cache.len() >= MAX_CACHED_PATTERNS {
        cache.clear();
    }
    cache.insert(pattern.to_string(), Arc::clone(&regex));
    Some(regex)
}

/// Returns the first pattern that matches `message`, or `None` if none do.
///
/// Runs against the raw message text — unlike `keywords::should_moderate`, it does
/// **not** apply obfuscation-resistant normalization (leet substitutions, look-alike
/// folding, flood collapsing). This condition is for structural patterns (repeated
/// characters, spacing, formatting) rather than words. Pattern compileability
/// is validated when the rule is saved, so an invalid pattern here is treated as
/// never matching rather than panicking.
pub fn should_moderate(message: &str, patterns: &[String]) -> Option<String> {
    patterns.iter().find_map(|pattern| {
        let regex = compiled(pattern)?;
        regex.is_match(message).then(|| pattern.clone())
    })
}
