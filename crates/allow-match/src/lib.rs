//! Structural finding-to-policy matching for cargo-allow.
//!
//! This crate evaluates source-tree findings against policy receipts, classifies
//! lifecycle and evidence outcomes, and fails closed on ambiguous or invalid
//! selectors. It only reasons over source-syntax findings and policy data
//! supplied by callers.

mod classification;
mod evaluation;
mod lifecycle;
mod locality;
mod location_drift;
mod messages;
mod mode;
mod scoring;

pub use evaluation::{MatchEvaluation, OccurrenceAccounting, evaluate, evaluate_detailed};
pub use locality::scoped_locality_reasons;
pub use messages::finding_location;
pub use mode::CheckMode;
pub use scoring::{MatchStrength, classify_match, explain_match_failure, score_match};

#[cfg(test)]
mod tests;
