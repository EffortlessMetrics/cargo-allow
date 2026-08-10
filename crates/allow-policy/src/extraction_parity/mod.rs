//! Extraction parity and cutover contracts (#2606).
//!
//! Report-only in Wave 0 PR5: parity case catalog and stage receipt schema.

mod compare;
mod config;
mod reachability;
mod validate;

pub use compare::{ParityComparison, ParityObservation, compare_observations, corpus_digest};
pub use config::{
    ExtractionParityCase, ExtractionParityRegistry, ExtractionStage, ParityComparisonResult,
    ParityDisposition, StageReceiptTemplate, parse_extraction_parity_registry,
    parse_extraction_parity_registry_at,
};
pub use reachability::{
    AuthorityKind, AuthorityNode, OldPathCase, OldPathDisposition, ReachabilityDiagnostic,
    ReachabilityDiagnosticKind, ReachabilityReport, validate_cutover_reachability,
    validate_duplicate_authority, validate_old_path_reachability,
};
pub use validate::{
    ParityDiagnostic, ParityDiagnosticKind, ParityReport, validate_extraction_parity_registry,
    validate_extraction_parity_registry_at,
};

#[cfg(test)]
mod tests;
