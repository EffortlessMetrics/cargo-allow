//! Retained command-case evidence for the accepted #3149 denominator.
//!
//! This reader does not execute the selected binary, scan a source tree, resolve
//! configuration, or grant release authority. A valid selected execution and a
//! complete migration are separate facts. The first reader admits only bounded
//! A-family observations; the full catalogue and unresolved runtime bindings
//! keep qualification Partial.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[path = "core_command_migration_detail.rs"]
mod detail;
#[path = "core_command_migration_presentation.rs"]
mod presentation;
#[path = "core_command_migration_readback.rs"]
mod readback;
#[path = "core_command_migration_reconcile.rs"]
mod reconcile;

pub(crate) use readback::{decode, read_input, validate_new_output};
pub(crate) use reconcile::reconcile;

// Pin the full external catalogue without a production include outside this
// package. The conformance test binds these bytes to the repository document;
// packaged readers receive the complete catalogue as an explicit input. The
// digest covers the accepted 107-case denominator plus the read-only
// human_receipt/presentation fixture specifications (#4724).
pub(crate) const CATALOGUE_DIGEST: &str =
    "sha256:v1:c6be62242a03b520695f5e8ffa28530374042e6ab0ec6c3f0fc2e0a59143775a";
pub(crate) const CATALOGUE_SCHEMA: &str = "cargo-allow.command-migration-catalogue.v1";
pub(crate) const CONTEXT_SCHEMA: &str = "cargo-allow.command-case-context.v1";
pub(crate) const BUNDLE_SCHEMA: &str = "cargo-allow.command-case-bundle.v1";
pub(crate) const ADMISSION_SCHEMA: &str = "cargo-allow.command-case-admission.v1";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dimension {
    pub(crate) id: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseSpec {
    pub(crate) id: String,
    pub(crate) family: String,
    pub(crate) command: String,
    pub(crate) scenario: String,
    pub(crate) applicability: Vec<String>,
    pub(crate) obligation: String,
    pub(crate) first_family_collector: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalogue {
    pub(crate) schema_id: String,
    pub(crate) schema_version: u32,
    pub(crate) authority: Vec<String>,
    pub(crate) dimensions: Vec<Dimension>,
    pub(crate) cases: Vec<CaseSpec>,
    pub(crate) first_family_fixtures: BTreeMap<String, FixtureSpec>,
    pub(crate) claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FixtureSpec {
    pub(crate) source: String,
    pub(crate) policy: Option<String>,
    pub(crate) require_clean: bool,
    pub(crate) output_failure: bool,
    pub(crate) source_findings: usize,
    pub(crate) parse_errors: usize,
    pub(crate) policy_valid: Option<bool>,
    pub(crate) check_fails: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Member {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
    pub(crate) digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BinaryProvenance {
    SourceBuild,
    SuppliedInstalledCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BinaryContext {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
    pub(crate) digest: String,
    pub(crate) tool_version: String,
    pub(crate) provenance: BinaryProvenance,
    pub(crate) source_generation: String,
    pub(crate) candidate_identity: Option<String>,
    pub(crate) install_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseContext {
    pub(crate) case_id: String,
    pub(crate) root: String,
    pub(crate) cwd: String,
    pub(crate) argv: Vec<String>,
    pub(crate) environment: BTreeMap<String, String>,
    pub(crate) source_snapshot_digest: String,
    pub(crate) fixture_commit: String,
    pub(crate) output_digests: BTreeMap<String, Option<String>>,
    pub(crate) policy_digest: Option<String>,
    pub(crate) config_path: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) profile: Option<String>,
    pub(crate) resolved_config_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpectedContext {
    pub(crate) schema_id: String,
    pub(crate) schema_version: u32,
    pub(crate) collection_id: String,
    pub(crate) catalogue_digest: String,
    pub(crate) binary: BinaryContext,
    pub(crate) cases: Vec<CaseContext>,
    /// Read-only A presentation/human-receipt case contexts (#4724). Absent
    /// for first-family-only collections, keeping those retained bytes
    /// unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) presentation_cases: Option<Vec<PresentationCaseContext>>,
}

/// One actual invocation route of a presentation case. `route` names the
/// presentation dimension (`human`, `json`, `quiet`, `file`, `tty`,
/// `receipt_only`, `receipt_only_tty`); `tty` records whether the retained
/// stdout/stderr flowed through a real pseudo-terminal.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationContext {
    pub(crate) route: String,
    pub(crate) tty: bool,
    pub(crate) root: String,
    pub(crate) cwd: String,
    pub(crate) argv: Vec<String>,
    pub(crate) environment: BTreeMap<String, String>,
    pub(crate) source_snapshot_digest: String,
    pub(crate) fixture_commit: String,
    pub(crate) output_digests: BTreeMap<String, Option<String>>,
    pub(crate) policy_digest: Option<String>,
    pub(crate) config_path: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) profile: Option<String>,
    pub(crate) resolved_config_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationCaseContext {
    pub(crate) case_id: String,
    pub(crate) invocations: Vec<InvocationContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessObservation {
    pub(crate) started_at_utc: String,
    pub(crate) finished_at_utc: String,
    pub(crate) started: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) launch_error: Option<String>,
    pub(crate) timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseExecution {
    pub(crate) context: CaseContext,
    pub(crate) process: ProcessObservation,
    pub(crate) before: Member,
    pub(crate) after: Member,
    pub(crate) stdout: Member,
    pub(crate) stderr: Member,
    pub(crate) detail: Option<Member>,
    pub(crate) summary: Option<Member>,
    pub(crate) receipt: Option<Member>,
    pub(crate) output_guard: Option<OutputGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutputGuard {
    pub(crate) before: Member,
    pub(crate) after: Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bundle {
    pub(crate) schema_id: String,
    pub(crate) schema_version: u32,
    pub(crate) collection_id: String,
    pub(crate) catalogue_digest: String,
    pub(crate) context_digest: String,
    pub(crate) binary: BinaryContext,
    pub(crate) binary_member: Member,
    pub(crate) binary_digest_after: String,
    pub(crate) cases: Vec<CaseExecution>,
    /// Read-only A presentation/human-receipt retained executions (#4724).
    /// Absent for first-family-only collections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) presentation_cases: Option<Vec<PresentationCaseExecution>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationExecution {
    pub(crate) context: InvocationContext,
    pub(crate) process: ProcessObservation,
    pub(crate) before: Member,
    pub(crate) after: Member,
    pub(crate) stdout: Member,
    pub(crate) stderr: Member,
    pub(crate) detail: Option<Member>,
    pub(crate) summary: Option<Member>,
    pub(crate) receipt: Option<Member>,
    pub(crate) output_guard: Option<OutputGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationCaseExecution {
    pub(crate) case_id: String,
    pub(crate) invocations: Vec<InvocationExecution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotEntry {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) mode: u32,
    pub(crate) size_bytes: u64,
    pub(crate) content: Option<Member>,
    pub(crate) link_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snapshot {
    pub(crate) entries: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SemanticValidity {
    Valid,
    Incomplete,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseAdmission {
    pub(crate) case_id: String,
    pub(crate) semantic_validity: SemanticValidity,
    pub(crate) process_observed: bool,
    pub(crate) detail_reconciled: bool,
    pub(crate) observed_result_class: Option<String>,
    pub(crate) observed_completeness: Option<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) binding_gaps: Vec<String>,
    /// Per-invocation presentation admission (#4724). Empty for first-family
    /// cases, which keep their existing emitted shape.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) routes: Vec<RouteAdmission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RouteAdmission {
    pub(crate) route: String,
    pub(crate) tty: bool,
    pub(crate) semantic_validity: SemanticValidity,
    pub(crate) process_observed: bool,
    pub(crate) observed_exit_code: Option<i32>,
    pub(crate) errors: Vec<String>,
    pub(crate) binding_gaps: Vec<String>,
}

/// The retained native command-case admission projection. Serialization keeps
/// the exact emitted shape; deserialization admits only that shape, so the
/// release-experience consumer can bind the original retained bytes without a
/// second projection type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Admission {
    pub(crate) schema_id: String,
    pub(crate) schema_version: u32,
    pub(crate) catalogue_digest: String,
    pub(crate) context_digest: String,
    pub(crate) bundle_digest: String,
    pub(crate) collection_id: String,
    pub(crate) semantic_validity: SemanticValidity,
    pub(crate) qualification: String,
    pub(crate) cases: Vec<CaseAdmission>,
    pub(crate) missing_cases: Vec<String>,
    pub(crate) dimensions: Vec<Dimension>,
    pub(crate) errors: Vec<String>,
    pub(crate) qualification_gaps: Vec<String>,
    pub(crate) claim_boundary: String,
}

#[cfg(test)]
#[path = "core_command_migration_presentation_tests.rs"]
mod presentation_tests;
