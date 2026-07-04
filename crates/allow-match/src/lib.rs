//! Structural finding-to-policy matching for cargo-allow.
//!
//! This crate evaluates source-tree findings against policy receipts, classifies
//! lifecycle and evidence outcomes, and fails closed on ambiguous or invalid
//! selectors. It only reasons over source-syntax findings and policy data
//! supplied by callers.

mod classification;
mod evaluation;
mod lifecycle;
mod location_drift;
mod messages;
mod mode;
mod scoring;

pub use evaluation::evaluate;
pub use messages::finding_location;
pub use mode::CheckMode;
pub use scoring::{MatchStrength, classify_match, score_match};

#[cfg(test)]
mod tests;
