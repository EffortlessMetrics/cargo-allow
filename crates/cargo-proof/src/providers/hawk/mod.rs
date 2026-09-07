//! Hawk proof provider (#2555, absorbed into cargo-proof #2938).
mod adapter;
mod analysis_receipt;
mod finding_mapping;
mod receipt_currentness;
mod source_anchor_resolution;

pub use adapter::{HAWK_PROOF_PROVIDER_ID, HawkProofProviderV1};
pub use analysis_receipt::{
    HAWK_ANALYSIS_RECEIPT_SCHEMA_ID, HawkAnalysisReceiptError, HawkAnalysisReceiptV1,
    HawkExecutionModeV1, HawkFindingV1, validate_hawk_analysis_receipt,
};
pub use finding_mapping::{
    HAWK_FINDING_RESULT_SCHEMA_ID, HawkFindingResultV1, HawkResultClassV1, map_hawk_finding,
    map_missing_finding,
};
pub use receipt_currentness::{
    HAWK_RECEIPT_CURRENTNESS_SCHEMA_ID, HawkReceiptCurrentnessReportV1,
    HawkReceiptCurrentnessStatusV1, evaluate_hawk_receipt_currentness,
};
pub use source_anchor_resolution::{
    HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID, HawkSourceAnchorResolutionV1,
    SourceAnchorResolutionClassV1, SourceAnchorResolutionError, resolve_source_anchor,
};
