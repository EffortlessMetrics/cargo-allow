//! Core data model for cargo-allow source-tree exception governance.
//!
//! This crate defines the shared finding, policy-entry, selector, lifecycle,
//! match-outcome, path-normalization, and stable fingerprint primitives used by
//! the cargo-allow crate family. It does not scan source files, invoke Cargo,
//! compile code, or execute repository artifacts.

mod actionable_diagnostic;
mod capped_read;
mod companion_family;
mod date;
mod error;
mod finding;
mod fingerprint;
mod json;
mod lane_posture;
mod ledger_posture;
mod ledger_provenance;
mod policy;
mod source_tree_path;
pub use actionable_diagnostic::{
    ActionApplicability, ActionKind, CargoAllowActionV1, CargoAllowDiagnosticBatchV1,
    CargoAllowDiagnosticV1, DIAGNOSTIC_KERNEL_SCHEMA, DiagnosticConfidence, DiagnosticResultClass,
    DiagnosticSeverity, MissingObligation, PartialDataBoundary, PositionBase, RelatedLocation,
    RelatedRole, RequiredProof, RulePosture, SourceEncoding, SourcePosition, SourceProvenance,
    SourceRange,
};
pub use capped_read::{
    CappedReadError, SOURCE_FILE_READ_MAX_BYTES, read_file_capped, read_file_capped_with_limit,
    read_text_file_capped, read_text_file_capped_with_limit,
};
pub use companion_family::{REPOSITORY_WIDE_FAMILIES, is_repository_wide_family};
pub use date::SimpleDate;
pub use error::{
    CargoAllowDiagnostic, CargoAllowDiagnosticSeverity, CargoAllowError, CargoAllowErrorKind,
    CargoAllowErrorLocation, CargoAllowResult,
};
pub use finding::{
    Finding, FindingKind, MAX_IDENTITY_FIELD_LEN, STRUCTURAL_IDENTITY_SCHEMA_ID, Span,
    StructuralIdentity, finding_identity_key,
};
pub use fingerprint::{
    allow_entry_content_fingerprint, normalize_snippet, sha256_v1_bytes, stable_hash_hex,
};
pub use json::json_escape;
pub use lane_posture::{
    LaneConfig, LaneEnforcementMode, effective_lane_posture_for_findings,
    lane_enforcement_mode_for_kind,
};
pub use ledger_posture::{LedgerPosture, NetPosture, PostureDelta, PresenceMovement};
pub use ledger_provenance::LedgerProvenance;
pub use policy::{
    AllowConfig, AllowEntry, LastSeen, Lifecycle, MatchOutcome, MatchStatus, POLICY_NAME,
    Requirements, SUPPORTED_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION_ALIAS, Selector,
    WorkspaceConfig, WorkspaceMode,
};
pub use source_tree_path::{
    GLOB_MATCH_MAX_STEPS, allow_entry_broad_scope, glob_matches, glob_matches_str, normalize_path,
    source_tree_path_is_ignored, source_tree_path_matches_filter, source_tree_scope_has_wildcard,
};

#[cfg(test)]
mod tests;
