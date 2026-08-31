//! Typed manifest-to-lockfile dependency graph delta contracts (issue #3920).
//!
//! The contract compares one exact base/head pair of Cargo manifest sets and
//! `Cargo.lock` texts that the caller supplies. It never invokes Cargo, never
//! resolves dependencies, and never touches the network; parsing is limited to
//! syntax-visible manifest `[dependencies]`-family tables and Cargo.lock
//! `[[package]]` records. The receipt binds repository, commit, tree,
//! manifest-set digest, and lockfile digest identities so a stale receipt
//! cannot silently remain current when any input moves.
//!
//! Classification is a closed vocabulary: every movement either lands in one
//! of the issue-listed kinds or in [`DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure`].
//! Missing, malformed, empty, and zero-denominator inputs are reported as
//! instrument failures and can never be classified as
//! [`DependencyGraphDeltaKindV1::NoSemanticGraphChange`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) mod classify;
pub(crate) mod inputs;

/// Schema identity for the dependency graph delta receipt family.
pub const DEPENDENCY_GRAPH_DELTA_V1_SCHEMA_ID: &str = "cargo-allow.dependency-graph-delta.v1";

/// Current schema version of the dependency graph delta receipt.
pub const DEPENDENCY_GRAPH_DELTA_V1_SCHEMA_VERSION: u32 = 1;

/// Closed movement vocabulary for one dependency graph delta row.
///
/// Declaration order is the canonical row ordering, matching the issue
/// vocabulary order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphDeltaKindV1 {
    DirectRequirementAdded,
    DirectRequirementRemoved,
    DirectRequirementRaised,
    DirectRequirementLowered,
    RequirementRangeBroadened,
    RequirementRangeNarrowed,
    LockOnlyResolutionChanged,
    PackageAdded,
    PackageRemoved,
    PackageUpgraded,
    PackageDowngraded,
    SourceOrChecksumChanged,
    FeatureActivationChanged,
    DuplicateVersionMovement,
    TargetOrDependencyClassChanged,
    ManifestLockMismatch,
    NoSemanticGraphChange,
    UnsupportedOrInstrumentFailure,
}

impl DependencyGraphDeltaKindV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectRequirementAdded => "direct_requirement_added",
            Self::DirectRequirementRemoved => "direct_requirement_removed",
            Self::DirectRequirementRaised => "direct_requirement_raised",
            Self::DirectRequirementLowered => "direct_requirement_lowered",
            Self::RequirementRangeBroadened => "requirement_range_broadened",
            Self::RequirementRangeNarrowed => "requirement_range_narrowed",
            Self::LockOnlyResolutionChanged => "lock_only_resolution_changed",
            Self::PackageAdded => "package_added",
            Self::PackageRemoved => "package_removed",
            Self::PackageUpgraded => "package_upgraded",
            Self::PackageDowngraded => "package_downgraded",
            Self::SourceOrChecksumChanged => "source_or_checksum_changed",
            Self::FeatureActivationChanged => "feature_activation_changed",
            Self::DuplicateVersionMovement => "duplicate_version_movement",
            Self::TargetOrDependencyClassChanged => "target_or_dependency_class_changed",
            Self::ManifestLockMismatch => "manifest_lock_mismatch",
            Self::NoSemanticGraphChange => "no_semantic_graph_change",
            Self::UnsupportedOrInstrumentFailure => "unsupported_or_instrument_failure",
        }
    }
}

/// Edge class of a dependency requirement as declared in a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphEdgeClassV1 {
    Normal,
    Dev,
    Build,
}

impl DependencyGraphEdgeClassV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
        }
    }
}

/// Verdict for one dependency graph delta evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphDeltaVerdictV1 {
    /// Both sides parsed and every movement is classified.
    Complete,
    /// At least one input was missing, malformed, empty, or unparseable.
    /// Rows are restricted to failure descriptions.
    InstrumentFailure,
}

/// Caller-supplied inputs for one side of the comparison.
///
/// The caller (CI producer or test fixture) supplies exact manifest texts and
/// the `Cargo.lock` text for the side's commit/tree. No tool invocation
/// happens here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphSideInputV1 {
    pub commit: String,
    pub tree: String,
    /// Manifest path (repository-relative, `/`-separated) to manifest text.
    pub manifests: BTreeMap<String, String>,
    /// `Cargo.lock` text for the side, when the product declares one.
    pub lockfile: Option<String>,
}

/// Computed identity of one side, bound into the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphSideIdentityV1 {
    pub commit: String,
    pub tree: String,
    pub manifest_count: usize,
    /// SHA-256 over the sorted (path, length, text) manifest stream.
    pub manifest_set_digest: String,
    /// SHA-256 over the lockfile text, `None` when the side has no lockfile.
    pub lockfile_digest: Option<String>,
}

/// Request for one exact base/head dependency graph delta evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphDeltaRequestV1 {
    pub repository: String,
    /// Selected product/package-set denominator. Ambient workspace unions are
    /// not implied by this field.
    pub product: String,
    /// Selected target identity for the denominator.
    pub target: String,
    /// Bounded Cargo/tool identity when the producer supplies one.
    pub cargo_tool_identity: Option<String>,
    /// Feature configuration identity (#3905) where applicable.
    pub feature_configuration: Option<String>,
    pub base: DependencyGraphSideInputV1,
    pub head: DependencyGraphSideInputV1,
}

/// One classified dependency graph movement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DependencyGraphDeltaRowV1 {
    pub kind: DependencyGraphDeltaKindV1,
    /// Cargo package name (lockfile spelling) the row is about.
    pub package: String,
    pub dependency_class: DependencyGraphEdgeClassV1,
    /// Target-specific requirement table name, empty when not target-specific.
    pub target: String,
    /// Manifest that rooted the movement, when rooted in a manifest.
    pub manifest_path: Option<String>,
    pub base_version: Option<String>,
    pub head_version: Option<String>,
    pub base_requirement: Option<String>,
    pub head_requirement: Option<String>,
    pub base_source: Option<String>,
    pub head_source: Option<String>,
    /// Stable snake_case detail tokens; never a human free-form sentence.
    pub detail: String,
}

/// Deterministic receipt for one exact base/head dependency graph delta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphDeltaReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub product: String,
    pub target: String,
    pub cargo_tool_identity: Option<String>,
    pub feature_configuration: Option<String>,
    pub base: DependencyGraphSideIdentityV1,
    pub head: DependencyGraphSideIdentityV1,
    pub verdict: DependencyGraphDeltaVerdictV1,
    pub rows: Vec<DependencyGraphDeltaRowV1>,
    pub claim_boundary: Vec<String>,
    pub limitations: Vec<String>,
}

impl DependencyGraphDeltaReceiptV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = DEPENDENCY_GRAPH_DELTA_V1_SCHEMA_ID;
    pub const CURRENT_SCHEMA_VERSION: u32 = DEPENDENCY_GRAPH_DELTA_V1_SCHEMA_VERSION;
}

/// Evaluate one request into a deterministic receipt.
///
/// The function is pure: identical inputs always produce identical receipts,
/// independent of manifest key order or lockfile record order.
pub fn dependency_graph_delta(
    request: &DependencyGraphDeltaRequestV1,
) -> DependencyGraphDeltaReceiptV1 {
    let outcome = classify::classify_request(request);
    let mut rows = outcome.rows;
    rows.sort();
    let verdict = if outcome.instrument_failure {
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    } else {
        DependencyGraphDeltaVerdictV1::Complete
    };
    if verdict == DependencyGraphDeltaVerdictV1::Complete && rows.is_empty() {
        rows.push(DependencyGraphDeltaRowV1 {
            kind: DependencyGraphDeltaKindV1::NoSemanticGraphChange,
            package: request.product.clone(),
            dependency_class: DependencyGraphEdgeClassV1::Normal,
            target: String::new(),
            manifest_path: None,
            base_version: None,
            head_version: None,
            base_requirement: None,
            head_requirement: None,
            base_source: None,
            head_source: None,
            detail: "no_manifest_or_lockfile_semantic_movement".to_string(),
        });
    }
    DependencyGraphDeltaReceiptV1 {
        schema_id: DependencyGraphDeltaReceiptV1::CURRENT_SCHEMA_ID.to_string(),
        schema_version: DependencyGraphDeltaReceiptV1::CURRENT_SCHEMA_VERSION,
        repository: request.repository.clone(),
        product: request.product.clone(),
        target: request.target.clone(),
        cargo_tool_identity: request.cargo_tool_identity.clone(),
        feature_configuration: request.feature_configuration.clone(),
        base: side_identity(&request.base),
        head: side_identity(&request.head),
        verdict,
        rows,
        claim_boundary: claim_boundary(),
        limitations: limitations(),
    }
}

/// Compute the bound identity for one side's supplied inputs.
pub fn side_identity(input: &DependencyGraphSideInputV1) -> DependencyGraphSideIdentityV1 {
    DependencyGraphSideIdentityV1 {
        commit: input.commit.clone(),
        tree: input.tree.clone(),
        manifest_count: input.manifests.len(),
        manifest_set_digest: manifest_set_digest(&input.manifests),
        lockfile_digest: input
            .lockfile
            .as_ref()
            .map(|text| sha256_hex(text.as_bytes())),
    }
}

/// SHA-256 over the canonical manifest-set stream: entries in path order,
/// each framed as `path \0 byte_length \0 text`.
fn manifest_set_digest(manifests: &BTreeMap<String, String>) -> String {
    let mut bytes = Vec::new();
    for (path, text) in manifests {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        let length = text.len().to_string();
        bytes.extend_from_slice(length.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(text.as_bytes());
    }
    sha256_hex(&bytes)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn claim_boundary() -> Vec<String> {
    vec![
        "exact_base_head_manifest_and_lockfile_text_comparison".to_string(),
        "closed_movement_vocabulary_with_explicit_instrument_failure".to_string(),
        "identity_bound_to_commits_trees_and_input_digests".to_string(),
        "no_cargo_invocation_network_or_dependency_resolution".to_string(),
        "movement_visibility_never_converted_into_safety_proof".to_string(),
    ]
}

fn limitations() -> Vec<String> {
    vec![
        "range_and_prerelease_semantics_approximated_from_manifest_text".to_string(),
        "product_scoped_closure_requires_bounded_cargo_metadata_for_full_fidelity".to_string(),
        "does_not_prove_dependency_safety_or_behavioral_compatibility".to_string(),
        "does_not_edit_manifests_lockfiles_or_dependency_policy".to_string(),
    ]
}
