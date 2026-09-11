mod filter;

#[cfg(test)]
mod tests;

pub use filter::{should_moderate_blank, should_moderate_invisible};
