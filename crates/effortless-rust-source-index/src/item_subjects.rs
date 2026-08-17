//! Exact structural Rust item identities for cross-provider evidence joins (#3607).
//!
//! This module deliberately stops at source-structural facts. It does not
//! establish that an item compiled, is live, is dead, is externally consumed,
//! or is safe to change.

mod model;
mod resolve;

pub use model::*;
pub use resolve::*;

#[cfg(test)]
mod tests;
