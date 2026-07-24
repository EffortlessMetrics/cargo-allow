//! Hawk analysis receipt validation and finding mapping (#2555).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-adapter-hawk` validates captured Hawk JSON reports, preserves finding
//! result classes, and resolves intent source anchors without importing
//! rustc-private code or Hawk crates. It does not scan source files, does not
//! invoke Cargo, compile code, execute repository code, spawn processes, or
//! depend on intent crates.

mod analysis_receipt;
mod analysis_receipt_surface;
mod boundary;
mod finding_mapping;
mod finding_mapping_surface;
mod hawk_adapter;
mod hawk_adapter_surface;
mod parity;
mod receipt_currentness;
mod receipt_currentness_surface;
mod source_anchor_resolution;
mod source_anchor_resolution_surface;

#[cfg(test)]
mod tests;

pub use analysis_receipt::{
    HAWK_ANALYSIS_RECEIPT_SCHEMA_ID, HawkAnalysisReceiptError, HawkAnalysisReceiptV1,
    HawkExecutionModeV1, HawkFindingV1, validate_hawk_analysis_receipt,
};
pub use analysis_receipt_surface::AnalysisReceiptSurface;
pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use finding_mapping::{
    HAWK_FINDING_RESULT_SCHEMA_ID, HawkFindingResultV1, HawkResultClassV1, map_hawk_finding,
    map_missing_finding,
};
pub use finding_mapping_surface::FindingMappingSurface;
pub use hawk_adapter::{HAWK_PROOF_PROVIDER_ID, HawkProofProviderV1};
pub use hawk_adapter_surface::HawkAdapterSurface;
pub use parity::{
    AnalysisReceiptParityContract, FindingMappingParityContract,
    load_analysis_receipt_parity_contract, load_finding_mapping_parity_contract,
    parity_contract_path, parity_contract_paths,
};
pub use receipt_currentness::{
    HAWK_RECEIPT_CURRENTNESS_SCHEMA_ID, HawkReceiptCurrentnessReportV1,
    HawkReceiptCurrentnessStatusV1, evaluate_hawk_receipt_currentness,
};
pub use receipt_currentness_surface::ReceiptCurrentnessSurface;
pub use source_anchor_resolution::{
    HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID, HawkSourceAnchorResolutionV1,
    SourceAnchorResolutionClassV1, SourceAnchorResolutionError, resolve_source_anchor,
};
pub use source_anchor_resolution_surface::SourceAnchorResolutionSurface;
