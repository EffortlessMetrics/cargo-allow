//! Provider-neutral repository identity and transport envelopes for the cargo-allow
//! three-product extraction (#2582).
//!
//! This crate defines shared source-tree transport contracts used across cargo-allow,
//! cargo-intent, and cargo-proof. It does not scan source files, invoke Cargo,
//! compile code, execute repository artifacts, or access Git or the filesystem.

mod analysis_receipt;
mod canonical;
mod claim_boundary;
mod completeness;
mod currentness;
mod repository_snapshot;
mod result_class;
mod source_anchor;
mod source_view;

pub use analysis_receipt::{ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1};
pub use canonical::{canonical_json_bytes, stable_digest_hex, stable_digest_json};
pub use claim_boundary::ClaimBoundaryV1;
pub use completeness::CompletenessV1;
pub use currentness::CurrentnessV1;
pub use repository_snapshot::{
    REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotKindV1, RepositorySnapshotV1,
    ResolvedRevisionV1, SelectedPathIdentityV1,
};
pub use result_class::ResultClassV1;
pub use source_anchor::{SourceAnchorV1, SourceIdentityV1};
pub use source_view::{
    REPOSITORY_SOURCE_VIEW_SCHEMA_ID, RepositorySourceViewKindV1, RepositorySourceViewV1,
    SnapshotDiagnosticV1, SourceContentDigestV1, SourceEntryV1, SourceSelectionInputV1,
};

#[cfg(test)]
mod tests;
