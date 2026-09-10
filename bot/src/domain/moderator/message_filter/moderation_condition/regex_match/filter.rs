use regex::Regex;

/// Returns the first pattern that matches `message`, or `None` if none do.
///
/// Runs against the raw message text — unlike `keywords::should_moderate`, it does
/// **not** apply obfuscation-resistant normalization (leet substitutions, look-alike
/// folding, flood collapsing). This condition is for structural patterns (repeated
/// characters, spacing, formatting) rather than banned words. Pattern compileability
/// is validated when the rule is saved, so an invalid pattern here is treated as
/// never matching rather than panicking.
pub fn should_moderate(message: &str, patterns: &[String]) -> Option<String> {
    patterns.iter().find_map(|pattern| {
        let re = Regex::new(pattern).ok()?;
        re.is_match(message).then(|| pattern.clone())
    })
}
