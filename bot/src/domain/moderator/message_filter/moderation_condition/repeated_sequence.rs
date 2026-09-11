mod filter;

#[cfg(test)]
mod tests;

pub use filter::{MAX_SEQUENCE_LENGTH, should_moderate};
