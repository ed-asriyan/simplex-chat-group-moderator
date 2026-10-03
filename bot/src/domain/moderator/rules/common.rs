//! Code more than one condition or action needs, so that none of them has to
//! reach into another.
//!
//! Nothing here knows a condition or an action by name: a helper that needs
//! one belongs to that condition or action instead.

pub mod api_key;
pub mod api_retry;
pub mod checks;
pub mod domains;
pub mod invisible;
pub mod rate_limit;
pub mod screen_lines;
