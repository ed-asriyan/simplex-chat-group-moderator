//! The moderator context's ports: how the outside world drives it
//! (`inbound`), what it needs from the outside (`outbound`), and the types
//! both sides exchange (`types`).
//!
//! Everything is re-exported here, so callers keep using
//! `domain::moderator::ports::X` regardless of which file `X` lives in.

pub mod inbound;
pub mod outbound;
pub mod types;

pub use inbound::*;
pub use outbound::*;
pub use types::*;
