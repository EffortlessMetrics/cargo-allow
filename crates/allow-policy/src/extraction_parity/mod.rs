//! Extraction parity and cutover contracts (#2606).
//!
//! Report-only in Wave 0 PR5: parity case catalog and stage receipt schema.

mod compare;
mod config;
mod cutover_receipt;
mod producer;
mod reachability;
mod validate;

pub use compare::{ParityComparison, ParityObservation, compare_observations, corpus_digest};
pub use config::{
    ExtractionParityCase, ExtractionParityRegistry, ExtractionStage, ParityComparisonResult,
    ParityDisposition, StageReceiptTemplate, parse_extraction_parity_registry,
    parse_extraction_parity_registry_at,
};
pub use cutover_receipt::{
    CutoverReceiptDiagnostic, CutoverReceiptDiagnosticKind, EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID,
    EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION, ExtractionCutoverReceipt,
    parse_extraction_cutover_receipt, parse_extraction_cutover_receipt_at,
    validate_extraction_cutover_receipt,
};
pub use producer::{ExtractionCutoverReceiptEvidence, produce_extraction_cutover_receipt};
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
