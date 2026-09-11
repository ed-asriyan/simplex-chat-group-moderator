mod filter;

#[cfg(test)]
mod tests;

pub use filter::{should_moderate_characters, should_moderate_lines, should_moderate_words};
