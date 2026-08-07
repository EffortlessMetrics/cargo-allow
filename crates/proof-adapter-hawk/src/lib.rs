//! Hawk analysis receipt validation and finding mapping (#2555).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-adapter-hawk` validates captured Hawk JSON reports, preserves finding
//! result classes, and resolves intent source anchors without importing
//! rustc-private code or Hawk crates. It does not scan source files, does not
//! invoke Cargo, compile code, execute repository code, spawn processes, or
//! depend on intent crates.

mod analysis_receipt;
#[cfg(test)]
mod boundary;
mod finding_mapping;
mod hawk_adapter;
mod parity;
mod receipt_currentness;
mod source_anchor_resolution;

#[cfg(test)]
mod tests;

pub use analysis_receipt::{
    HAWK_ANALYSIS_RECEIPT_SCHEMA_ID, HawkAnalysisReceiptError, HawkAnalysisReceiptV1,
    HawkExecutionModeV1, HawkFindingV1, validate_hawk_analysis_receipt,
};
pub use finding_mapping::{
    HAWK_FINDING_RESULT_SCHEMA_ID, HawkFindingResultV1, HawkResultClassV1, map_hawk_finding,
    map_missing_finding,
};
pub use hawk_adapter::{HAWK_PROOF_PROVIDER_ID, HawkProofProviderV1};
pub use parity::{
    AnalysisReceiptParityContract, FindingMappingParityContract,
    load_analysis_receipt_parity_contract, load_finding_mapping_parity_contract,
    parity_contract_path, parity_contract_paths,
};
pub use receipt_currentness::{
    HAWK_RECEIPT_CURRENTNESS_SCHEMA_ID, HawkReceiptCurrentnessReportV1,
    HawkReceiptCurrentnessStatusV1, evaluate_hawk_receipt_currentness,
};
pub use source_anchor_resolution::{
    HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID, HawkSourceAnchorResolutionV1,
    SourceAnchorResolutionClassV1, SourceAnchorResolutionError, resolve_source_anchor,
};
