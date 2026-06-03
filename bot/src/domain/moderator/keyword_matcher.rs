/// Returns `true` if any non-empty keyword in `keywords` appears as a whole
/// word in `text`, ignoring case and surrounding punctuation/whitespace.
///
/// A "word" is a maximal run of alphanumeric characters (Unicode-aware).
/// A single-token keyword matches one such word. A multi-token keyword
/// (e.g. `"hello world"`) matches a contiguous sequence of words.
pub fn contains_keyword(text: &str, keywords: &[String]) -> bool {
    let words: Vec<String> = tokenize(text);
    if words.is_empty() {
        return false;
    }
    keywords
        .iter()
        .filter(|kw| !kw.trim().is_empty())
        .any(|kw| match_keyword(&words, kw))
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn match_keyword(haystack_words: &[String], keyword: &str) -> bool {
    let needle_words = tokenize(keyword);
    if needle_words.is_empty() || needle_words.len() > haystack_words.len() {
        return false;
    }
    haystack_words
        .windows(needle_words.len())
        .any(|window| window == needle_words.as_slice())
}

#[cfg(test)]
mod tests;
