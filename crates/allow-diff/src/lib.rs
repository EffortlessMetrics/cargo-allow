//! PR-posture and policy-diff helpers for cargo-allow.
//!
//! This crate compares source-tree findings and policy ledger entries so callers
//! can report new, removed, broadened, weakened, or improved exceptions. It also
//! exposes the exact Git index as a read-only commit-candidate source whose
//! identity is independent of unstaged worktree bytes. Candidate parsing uses
//! checked extraction rather than panic-prone indexing. The crate works from
//! repository text and Git object contents; it does not invoke Cargo metadata,
//! rustc, Clippy, build scripts, proc macros, or evidence tools.

mod finding;
mod movement;
mod policy;
mod policy_change;
mod policy_change_details;
mod policy_change_kind;
mod policy_compare;
mod policy_entry;
mod policy_entry_evidence;
mod policy_entry_identity;
mod policy_entry_lifecycle;
mod policy_entry_limits;
mod policy_entry_metadata;
mod policy_entry_scope;
mod policy_entry_selector;
mod policy_header;
mod policy_requirements;
mod policy_scope;
mod policy_selector;
mod policy_workspace;
mod result;
mod revision;
mod revision_git;

pub use finding::{
    FindingPostureChange, FindingPostureKind, finding_identity_key, finding_posture_changes,
};
pub use movement::{
    DiffLedgerMovementSummary, DiffMovementCounts, DiffPostureDeltaCounts, DiffRowClassification,
    classify_finding_posture_change, classify_policy_change, diff_ledger_movement_summary,
    entry_lane, entry_ledger_id, finding_posture_delta, finding_posture_movement,
    finding_posture_subject, policy_change_lane, policy_change_ledger_id, policy_change_movement,
    policy_change_posture_delta, policy_change_subject,
};
pub use policy::{policy_changes, policy_changes_from_git, policy_config_at_revision};
pub use policy_change::{
    EvidenceChange, EvidenceChangeField, ExceptionIdentityChange, ExceptionIdentityChangeField,
    LifecycleChange, LifecycleChangeField, MetadataChange, MetadataChangeField,
    OccurrenceLimitChange, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
    PolicyStatusChange, RequirementChange, RequirementChangeField, ScopeChange, ScopeChangeField,
    SelectorIdentityChange, SelectorPrecisionChange,
};
pub use policy_scope::selector_precision_score;
pub use result::retain_confident_finding_changes;
pub use result::{DiffResultClass, DiffScanCoverage, classify_diff_result};
pub use revision::{RevisionScanResult, findings_at_revision, scan_at_revision};
pub use revision_git::{changed_files, git_tracked_files_at_revision, read_file_at_revision};

#[cfg(test)]
mod tests;
