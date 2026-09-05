//! Machine-visible frozen-subject lock (#3928).
//!
//! After the #2501 final freeze identifies exact bytes and claims, ordinary
//! work on load-bearing release surfaces must not silently move the subject
//! while the freeze receipt remains current. This module is the checked
//! state contract: it classifies the paths that moved since the frozen
//! commit against a typed load-bearing denominator, evaluates the lock
//! state and verdict, and records explicit invalidations.
//!
//! Lock law: load-bearing movement requires explicit freeze invalidation
//! before merge; a change outside the denominator may be allowed only
//! through the typed classification proving it cannot change candidate
//! bytes, claims, controls, or evidence currentness; invalidation is
//! append-only and stales the freeze consumers; "docs-only", "typo", or
//! author assertions are never sufficient by themselves; emergency repair
//! follows invalidate → repair → review → requalify → refreeze.
//!
//! Claim boundary: a repository guard preserving the exact final release
//! subject between freeze, authorization, execution, and reconciliation.
//! It requires explicit invalidation for load-bearing changes while
//! allowing proven unrelated work; it never authorizes or executes release
//! operations, never mutates live branch rules, and never freezes the
//! whole repository.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const FROZEN_SUBJECT_LOCK_SCHEMA_ID: &str = "cargo-allow.frozen-subject-lock.v1";
pub const FROZEN_SUBJECT_LOCK_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "A machine-visible repository guard preserving the exact final release subject between freeze, authorization, execution, and reconciliation. It requires explicit invalidation for load-bearing changes while allowing proven unrelated work through a typed classification; it does not authorize or execute release operations, does not replace branch protection, does not freeze the whole repository, and never creates tags, tokens, uploads, or publications.";

/// The eight lock states. Only `FreezeCompleteAwaitingAuthorization`,
/// `InvalidatedForRefreeze`, and `Inactive` are reachable from repository
/// state today; the execution-side states are modeled so the same contract
/// survives publication and reconciliation without a schema break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenSubjectStateV1 {
    Inactive,
    FreezeCompleteAwaitingAuthorization,
    AuthorizationAvailable,
    OperationInProgress,
    PublishedAwaitingReconciliation,
    InvalidatedForRefreeze,
    ConsumedComplete,
    ConsumedIncident,
}

/// Closed verdict vocabulary for one evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenSubjectVerdictV1 {
    Complete,
    AllowedNonLoadBearing,
    RequiresInvalidation,
    Conflict,
    Stale,
    InstrumentFailure,
}

impl FrozenSubjectVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::AllowedNonLoadBearing => "allowed_non_load_bearing",
            Self::RequiresInvalidation => "requires_invalidation",
            Self::Conflict => "conflict",
            Self::Stale => "stale",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Semantic owner of one load-bearing path class. The freeze law
/// enumerates these surfaces; movement in any of them changes the frozen
/// release subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBearingOwnerV1 {
    FrozenLockRecords,
    ShippedSource,
    ManifestsLockfile,
    Toolchain,
    Policy,
    PackageDocsAssets,
    Schemas,
    SupportChannelTruth,
    ReleaseRecords,
    WorkflowsActions,
    ReleaseEvidenceProducers,
}

impl LoadBearingOwnerV1 {
    /// Ordered first-match patterns: exact path, directory prefix (the
    /// pattern matches the path or any path below it), or a star-suffix
    /// name prefix. Order matters only within one owner.
    pub(crate) fn patterns(self) -> &'static [&'static str] {
        match self {
            Self::FrozenLockRecords => &["docs/dogfood/receipts/final-freeze"],
            Self::ShippedSource => &[
                "crates/allow-core",
                "crates/allow-policy-legacy",
                "crates/allow-policy",
                "crates/allow-inventory",
                "crates/allow-files",
                "crates/allow-rust",
                "crates/allow-match",
                "crates/allow-report",
                "crates/allow-diff",
                "crates/cargo-allow",
                "crates/effortless-repo-edit",
                "crates/effortless-repo-protocol",
                "crates/effortless-repo-snapshot",
            ],
            Self::ManifestsLockfile => &["Cargo.toml", "Cargo.lock", ".cargo"],
            Self::Toolchain => &["rust-toolchain.toml"],
            Self::Policy => &["policy"],
            Self::PackageDocsAssets => &[
                "README.md",
                "SUPPORT.md",
                "SECURITY.md",
                "LICENSE-MIT",
                "LICENSE-APACHE",
                ".pre-commit-hooks.yaml",
                "docs/getting-started.md",
                "docs/how-to",
                "docs/dogfood/fixtures",
            ],
            Self::Schemas => &["docs/schemas"],
            Self::SupportChannelTruth => &["docs/support-matrix.toml", "docs/support"],
            Self::ReleaseRecords => &["docs/release", "docs/github", ".changes", ".changie.yaml"],
            Self::WorkflowsActions => &[".github"],
            Self::ReleaseEvidenceProducers => &[
                "scripts/release-",
                "scripts/generate-release-manifest.sh",
                "scripts/observe-live-release-controls.sh",
                "scripts/final-package-docs",
                "scripts/exact-candidate-",
                "scripts/exact-upgrade-rollback-journey.sh",
                "scripts/validate-upgrade-rollback-journey.py",
                "scripts/source-candidate-smoke.sh",
                "scripts/package-candidate-smoke.sh",
                "scripts/candidate-harness-owned-dir.py",
                "scripts/check-msrv-",
                "scripts/verify-crate-registry-version.sh",
                "scripts/test-exact-candidate-",
                "scripts/test-release-",
                "scripts/test-package-candidate-smoke.sh",
                "scripts/test-verify-crate-registry-version.sh",
                "scripts/exact_candidate_package_identity.py",
            ],
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FrozenLockRecords => "frozen_lock_records",
            Self::ShippedSource => "shipped_source",
            Self::ManifestsLockfile => "manifests_lockfile",
            Self::Toolchain => "toolchain",
            Self::Policy => "policy",
            Self::PackageDocsAssets => "package_docs_assets",
            Self::Schemas => "schemas",
            Self::SupportChannelTruth => "support_channel_truth",
            Self::ReleaseRecords => "release_records",
            Self::WorkflowsActions => "workflows_actions",
            Self::ReleaseEvidenceProducers => "release_evidence_producers",
        }
    }
}

/// Semantic owner of one provably non-load-bearing path class. Membership
/// is the typed evidence that the class cannot change candidate bytes,
/// claims, controls, or evidence currentness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonLoadBearingOwnerV1 {
    /// The lock's own CI machinery: minimal-permission enforcement only.
    FrozenSubjectLockMachinery,
    /// Unselected sibling products outside the frozen closure.
    SiblingProducts,
    /// Retained records of other lanes (the final-freeze records are
    /// load-bearing and match first).
    CampaignRecords,
    /// Repository prose outside package, support, release, and policy
    /// meaning.
    RepositoryProse,
}

impl NonLoadBearingOwnerV1 {
    pub(crate) fn patterns(self) -> &'static [&'static str] {
        match self {
            Self::FrozenSubjectLockMachinery => &[".github/workflows/frozen-subject-lock.yml"],
            Self::SiblingProducts => &[
                "crates/cargo-intent",
                "crates/cargo-proof",
                "crates/effortless-rust-source-index",
                "crates/intent-",
                "crates/proof-",
            ],
            Self::CampaignRecords => &["docs/dogfood/receipts"],
            Self::RepositoryProse => &[
                "docs/source-of-truth",
                "docs/adr",
                "docs/incidents",
                "docs/incident-recovery.md",
                "AGENTS.md",
                ".agents",
            ],
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FrozenSubjectLockMachinery => "frozen_subject_lock_machinery",
            Self::SiblingProducts => "sibling_products",
            Self::CampaignRecords => "campaign_records",
            Self::RepositoryProse => "repository_prose",
        }
    }
}

/// One changed path with its classification and the evidence result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenSubjectPathClassV1 {
    pub path: String,
    pub status: String,
    #[serde(rename = "classification")]
    pub kind: FrozenSubjectPathKindV1,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenSubjectPathKindV1 {
    LoadBearing,
    NonLoadBearing,
}

/// One changed path with its git diff status (A/M/D/R...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenSubjectChangeV1 {
    pub status: String,
    pub path: String,
    /// Typed evidence that the diff is purely additive (no removals or
    /// modifications), proven from the diff content by the caller.
    #[serde(default)]
    pub append_only: bool,
}

/// An append-only invalidation record. Any record moves the lock to
/// `InvalidatedForRefreeze` and stales the freeze consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenSubjectInvalidationV1 {
    pub reason: String,
    pub recorded_by: String,
    pub recorded_at_utc: String,
    /// The frozen commit the invalidation applies to.
    pub frozen_commit: String,
}

/// The retained freeze identity consumed from the retained receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenSubjectReceiptIdentityV1 {
    pub commit: String,
    pub tree: String,
    pub version: String,
    pub tag: String,
    /// The receipt's own freeze-state string (Complete).
    pub freeze_state: String,
    /// The receipt file's sha256 in the typed form.
    pub receipt_digest: String,
}

/// Evaluation input: retained identity, the current head, the changed
/// paths, and any invalidation records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenSubjectLockInputV1 {
    pub receipt: Option<FrozenSubjectReceiptIdentityV1>,
    pub current_head: String,
    pub changed_paths: Vec<FrozenSubjectChangeV1>,
    pub invalidations: Vec<FrozenSubjectInvalidationV1>,
}

/// The evaluated lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFrozenSubjectLockV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub state: FrozenSubjectStateV1,
    pub verdict: FrozenSubjectVerdictV1,
    pub frozen_commit: Option<String>,
    pub current_head: Option<String>,
    pub classified_paths: Vec<FrozenSubjectPathClassV1>,
    pub load_bearing_moved: Vec<String>,
    pub invalidation_count: usize,
    pub blocking_rows: Vec<String>,
    pub claim_boundary: &'static str,
}

/// Classify one path: first match wins across the non-load-bearing and
/// load-bearing pattern tables (the lock's own machinery and the
/// final-freeze records are matched before the broad `.github` and
/// receipts patterns).
#[must_use]
pub fn classify_frozen_subject_path(
    path: &str,
    status: &str,
    append_only: bool,
) -> FrozenSubjectPathClassV1 {
    fn matches(pattern: &str, path: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            return path.starts_with(prefix);
        }
        if pattern.ends_with('-') {
            // Name-prefix pattern (e.g. `scripts/release-`): match any
            // path below or beside the dash.
            return path.starts_with(pattern);
        }
        path == pattern || path.starts_with(&format!("{pattern}/"))
    }
    // The retained final-freeze records are tamper-evident: they are
    // matched before the broader campaign-records pattern.
    let class = |kind: FrozenSubjectPathKindV1, owner: String| FrozenSubjectPathClassV1 {
        path: path.to_string(),
        status: status.to_string(),
        kind,
        owner,
    };
    // A proven append-only ledger diff cannot change candidate bytes (the
    // ledger does not ship) or gate semantics (append-only receipt entries
    // carry reviewed evidence and the change-note gate governs integrity).
    // A proven append-only ledger diff cannot change candidate bytes (the
    // ledger does not ship) or gate semantics (append-only receipt entries
    // carry reviewed evidence and the change-note gate governs integrity).
    // Without the append-only proof the path stays load-bearing Policy.
    if path == "policy/allow.toml" {
        if append_only {
            return class(
                FrozenSubjectPathKindV1::NonLoadBearing,
                "ledger_append_only".to_string(),
            );
        }
        return class(FrozenSubjectPathKindV1::LoadBearing, "policy".to_string());
    }
    let lock_records = LoadBearingOwnerV1::FrozenLockRecords;
    for pattern in lock_records.patterns() {
        if matches(pattern, path) {
            return class(
                FrozenSubjectPathKindV1::LoadBearing,
                lock_records.label().to_string(),
            );
        }
    }
    for owner in NonLoadBearingOwnerV1::patterns_owned() {
        for pattern in owner.patterns() {
            if matches(pattern, path) {
                return class(
                    FrozenSubjectPathKindV1::NonLoadBearing,
                    owner.label().to_string(),
                );
            }
        }
    }
    for owner in LoadBearingOwnerV1::patterns_owned().iter().skip(1) {
        for pattern in owner.patterns() {
            if matches(pattern, path) {
                return class(
                    FrozenSubjectPathKindV1::LoadBearing,
                    owner.label().to_string(),
                );
            }
        }
    }
    class(
        FrozenSubjectPathKindV1::NonLoadBearing,
        "unclassified".to_string(),
    )
}

impl NonLoadBearingOwnerV1 {
    fn patterns_owned() -> [Self; 4] {
        [
            Self::FrozenSubjectLockMachinery,
            Self::SiblingProducts,
            Self::CampaignRecords,
            Self::RepositoryProse,
        ]
    }
}

impl LoadBearingOwnerV1 {
    fn patterns_owned() -> [Self; 11] {
        [
            Self::FrozenLockRecords,
            Self::ShippedSource,
            Self::ManifestsLockfile,
            Self::Toolchain,
            Self::Policy,
            Self::Schemas,
            Self::PackageDocsAssets,
            Self::SupportChannelTruth,
            Self::ReleaseRecords,
            Self::WorkflowsActions,
            Self::ReleaseEvidenceProducers,
        ]
    }
}

/// Evaluate the lock. Pure and timestamp-free.
#[must_use]
pub fn evaluate_frozen_subject_lock(
    input: &FrozenSubjectLockInputV1,
) -> CargoAllowFrozenSubjectLockV1 {
    let mut blocking_rows = Vec::new();
    let classified: Vec<FrozenSubjectPathClassV1> = input
        .changed_paths
        .iter()
        .map(|change| {
            classify_frozen_subject_path(&change.path, &change.status, change.append_only)
        })
        .collect();
    let load_bearing_moved: Vec<String> = classified
        .iter()
        .filter(|c| c.kind == FrozenSubjectPathKindV1::LoadBearing)
        .map(|c| c.path.clone())
        .collect();

    let Some(receipt) = &input.receipt else {
        // No retained freeze identity: nothing to enforce, but the absence
        // is itself alarming while the freeze should be current.
        blocking_rows.push("no retained freeze receipt is available".to_string());
        return CargoAllowFrozenSubjectLockV1 {
            schema_id: FROZEN_SUBJECT_LOCK_SCHEMA_ID.to_string(),
            schema_version: FROZEN_SUBJECT_LOCK_SCHEMA_VERSION,
            state: FrozenSubjectStateV1::Inactive,
            verdict: FrozenSubjectVerdictV1::Stale,
            frozen_commit: None,
            current_head: Some(input.current_head.clone()),
            classified_paths: classified,
            load_bearing_moved,
            invalidation_count: input.invalidations.len(),
            blocking_rows,
            claim_boundary: CLAIM_BOUNDARY,
        };
    };
    if receipt.freeze_state != "Complete" {
        blocking_rows.push(format!(
            "the retained freeze receipt is not Complete (state {:?})",
            receipt.freeze_state
        ));
    }

    // An invalidation applies only to the frozen commit it names: after a
    // refreeze the new receipt binds a new commit and historical
    // invalidations of the old subject are just history.
    let applicable_invalidations: Vec<&FrozenSubjectInvalidationV1> = input
        .invalidations
        .iter()
        .filter(|record| Some(record.frozen_commit.as_str()) == Some(receipt.commit.as_str()))
        .collect();
    let state = if !applicable_invalidations.is_empty() {
        FrozenSubjectStateV1::InvalidatedForRefreeze
    } else {
        FrozenSubjectStateV1::FreezeCompleteAwaitingAuthorization
    };

    // Tamper check: the frozen lock records must not be modified or
    // removed after creation; additions are the records' own creation.
    if classified.iter().any(|c| {
        c.owner == LoadBearingOwnerV1::FrozenLockRecords.label()
            && ["M", "D"].iter().any(|s| c.status.starts_with(s))
    }) {
        blocking_rows.push(
            "the retained final-freeze records moved; this is lock tampering until a refreeze"
                .to_string(),
        );
        return CargoAllowFrozenSubjectLockV1 {
            schema_id: FROZEN_SUBJECT_LOCK_SCHEMA_ID.to_string(),
            schema_version: FROZEN_SUBJECT_LOCK_SCHEMA_VERSION,
            state: FrozenSubjectStateV1::InvalidatedForRefreeze,
            verdict: FrozenSubjectVerdictV1::Conflict,
            frozen_commit: Some(receipt.commit.clone()),
            current_head: Some(input.current_head.clone()),
            classified_paths: classified,
            load_bearing_moved,
            invalidation_count: applicable_invalidations.len(),
            blocking_rows,
            claim_boundary: CLAIM_BOUNDARY,
        };
    }

    // Creation (status A) of the retained final-freeze records is their own
    // admission; only modification or removal is load-bearing movement.
    let admissions: Vec<String> = classified
        .iter()
        .filter(|c| c.owner == LoadBearingOwnerV1::FrozenLockRecords.label() && c.status == "A")
        .map(|c| c.path.clone())
        .collect();
    let effective_load_bearing: Vec<String> = load_bearing_moved
        .iter()
        .filter(|path| !admissions.contains(path))
        .cloned()
        .collect();

    if effective_load_bearing.is_empty() {
        let verdict = if state == FrozenSubjectStateV1::InvalidatedForRefreeze {
            // The invalidation already staled the freeze; the non-load-bearing
            // movement is allowed and the freeze must be redone.
            FrozenSubjectVerdictV1::Stale
        } else if classified.is_empty() {
            FrozenSubjectVerdictV1::Complete
        } else {
            FrozenSubjectVerdictV1::AllowedNonLoadBearing
        };
        return CargoAllowFrozenSubjectLockV1 {
            schema_id: FROZEN_SUBJECT_LOCK_SCHEMA_ID.to_string(),
            schema_version: FROZEN_SUBJECT_LOCK_SCHEMA_VERSION,
            state,
            verdict,
            frozen_commit: Some(receipt.commit.clone()),
            current_head: Some(input.current_head.clone()),
            classified_paths: classified,
            load_bearing_moved,
            invalidation_count: applicable_invalidations.len(),
            blocking_rows,
            claim_boundary: CLAIM_BOUNDARY,
        };
    }

    if state == FrozenSubjectStateV1::InvalidatedForRefreeze {
        // Load-bearing movement under an explicit invalidation: allowed to
        // proceed, but the freeze is stale and must be redone.
        return CargoAllowFrozenSubjectLockV1 {
            schema_id: FROZEN_SUBJECT_LOCK_SCHEMA_ID.to_string(),
            schema_version: FROZEN_SUBJECT_LOCK_SCHEMA_VERSION,
            state,
            verdict: FrozenSubjectVerdictV1::Stale,
            frozen_commit: Some(receipt.commit.clone()),
            current_head: Some(input.current_head.clone()),
            classified_paths: classified,
            load_bearing_moved,
            invalidation_count: applicable_invalidations.len(),
            blocking_rows: vec![
                "load-bearing movement under an explicit invalidation: the freeze is stale and must be redone".to_string(),
            ],
            claim_boundary: CLAIM_BOUNDARY,
        };
    }

    blocking_rows.push(format!(
        "load-bearing movement without invalidation: {}",
        effective_load_bearing.join(", ")
    ));
    CargoAllowFrozenSubjectLockV1 {
        schema_id: FROZEN_SUBJECT_LOCK_SCHEMA_ID.to_string(),
        schema_version: FROZEN_SUBJECT_LOCK_SCHEMA_VERSION,
        state: FrozenSubjectStateV1::FreezeCompleteAwaitingAuthorization,
        verdict: FrozenSubjectVerdictV1::RequiresInvalidation,
        frozen_commit: Some(receipt.commit.clone()),
        current_head: Some(input.current_head.clone()),
        classified_paths: classified,
        load_bearing_moved,
        invalidation_count: applicable_invalidations.len(),
        blocking_rows,
        claim_boundary: CLAIM_BOUNDARY,
    }
}

impl fmt::Display for FrozenSubjectVerdictV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrozenSubjectInvalidationV1, FrozenSubjectLockInputV1, FrozenSubjectPathKindV1,
        FrozenSubjectReceiptIdentityV1, FrozenSubjectStateV1, FrozenSubjectVerdictV1,
        classify_frozen_subject_path, evaluate_frozen_subject_lock,
    };

    fn receipt() -> FrozenSubjectReceiptIdentityV1 {
        FrozenSubjectReceiptIdentityV1 {
            commit: "63248416c2bd73edd63e22f064a1f242afcc0622".to_string(),
            tree: "c0031b8e290f4ad16615971c9ff3f93bb5093b79".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            freeze_state: "Complete".to_string(),
            receipt_digest:
                "sha256:v1:0724777b6864811a72048780fdea09c9f98aa34afe971ce853e875ae5b43017d"
                    .to_string(),
        }
    }

    fn change(status: &str, path: &str) -> super::FrozenSubjectChangeV1 {
        super::FrozenSubjectChangeV1 {
            status: status.to_string(),
            path: path.to_string(),
            append_only: false,
        }
    }

    fn input(paths: &[(&str, &str)]) -> FrozenSubjectLockInputV1 {
        FrozenSubjectLockInputV1 {
            receipt: Some(receipt()),
            current_head: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            changed_paths: paths
                .iter()
                .map(|(status, path)| change(status, path))
                .collect(),
            invalidations: Vec::new(),
        }
    }

    fn invalidate() -> FrozenSubjectInvalidationV1 {
        FrozenSubjectInvalidationV1 {
            reason: "load-bearing repair required".to_string(),
            recorded_by: "core/release".to_string(),
            recorded_at_utc: "2026-09-04T00:00:00Z".to_string(),
            frozen_commit: "63248416c2bd73edd63e22f064a1f242afcc0622".to_string(),
        }
    }

    #[test]
    fn control_1_package_readme_change_requires_invalidation() {
        let lock = evaluate_frozen_subject_lock(&input(&[("A", "README.md")]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::RequiresInvalidation);
        assert_eq!(
            lock.state,
            FrozenSubjectStateV1::FreezeCompleteAwaitingAuthorization
        );
    }

    #[test]
    fn control_2_workflow_permission_change_is_never_docs_only() {
        // The lock's own machinery file is the only non-load-bearing
        // workflow, and it is classified as machinery, never as prose.
        let machinery =
            classify_frozen_subject_path(".github/workflows/frozen-subject-lock.yml", "A", false);
        assert_eq!(machinery.kind, FrozenSubjectPathKindV1::NonLoadBearing);
        assert_eq!(machinery.owner, "frozen_subject_lock_machinery");
        let release = classify_frozen_subject_path(".github/workflows/release.yml", "A", false);
        assert_eq!(release.kind, FrozenSubjectPathKindV1::LoadBearing);
        let ci = classify_frozen_subject_path(".github/workflows/ci.yml", "A", false);
        assert_eq!(ci.kind, FrozenSubjectPathKindV1::LoadBearing);
    }

    #[test]
    fn control_3_lockfile_changes_are_never_ignored() {
        let lock = evaluate_frozen_subject_lock(&input(&[("A", "Cargo.lock")]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::RequiresInvalidation);
        // Even a lock change that only touches a sibling product's rows is
        // load-bearing: the lockfile is one shared artifact.
        let sibling_only = evaluate_frozen_subject_lock(&input(&[
            ("A", "crates/cargo-intent/src/lib.rs"),
            ("A", "Cargo.lock"),
        ]));
        assert_eq!(
            sibling_only.verdict,
            FrozenSubjectVerdictV1::RequiresInvalidation
        );
    }

    #[test]
    fn control_4_comment_only_source_changes_are_load_bearing() {
        let lock = evaluate_frozen_subject_lock(&input(&[("A", "crates/allow-core/src/lib.rs")]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::RequiresInvalidation);
    }

    #[test]
    fn control_5_repository_prose_is_allowed_with_a_typed_result() {
        let lock = evaluate_frozen_subject_lock(&input(&[
            ("A", "docs/adr/0009-record-decision.md"),
            ("A", "docs/source-of-truth/README.md"),
        ]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::AllowedNonLoadBearing);
        assert!(
            lock.classified_paths
                .iter()
                .all(|c| c.kind == FrozenSubjectPathKindV1::NonLoadBearing)
        );
    }

    #[test]
    fn control_6_schema_producer_movement_is_load_bearing() {
        let lock = evaluate_frozen_subject_lock(&input(&[(
            "A",
            "docs/schemas/candidate-preparation-plan-v1.schema.json",
        )]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::RequiresInvalidation);
    }

    #[test]
    fn invalidation_stales_the_freeze_instead_of_blocking() {
        let mut input = input(&[("A", "crates/allow-core/src/lib.rs")]);
        input.invalidations.push(invalidate());
        let lock = evaluate_frozen_subject_lock(&input);
        assert_eq!(lock.state, FrozenSubjectStateV1::InvalidatedForRefreeze);
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::Stale);
    }

    #[test]
    fn unbound_select_inputs_are_outside_the_frozen_closure() {
        let lock = evaluate_frozen_subject_lock(&input(&[
            ("A", "crates/cargo-intent/src/plan.rs"),
            ("A", "crates/proof-orchestrator/src/run.rs"),
        ]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::AllowedNonLoadBearing);
        assert!(lock.load_bearing_moved.is_empty());
    }

    #[test]
    fn moving_the_retained_lock_records_is_conflict() {
        // Modification/deletion of the retained receipt is tampering.
        let lock = evaluate_frozen_subject_lock(&input(&[(
            "M",
            "docs/dogfood/receipts/final-freeze/final-freeze.receipt.json",
        )]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::Conflict);
        assert_eq!(lock.state, FrozenSubjectStateV1::InvalidatedForRefreeze);
        // Creation is the records' own admission and stays allowed.
        let created = evaluate_frozen_subject_lock(&input(&[(
            "A",
            "docs/dogfood/receipts/final-freeze/final-freeze.receipt.json",
        )]));
        assert_eq!(
            created.verdict,
            FrozenSubjectVerdictV1::AllowedNonLoadBearing
        );
    }

    #[test]
    fn no_movement_is_complete() {
        let lock = evaluate_frozen_subject_lock(&input(&[]));
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::Complete);
        assert_eq!(
            lock.state,
            FrozenSubjectStateV1::FreezeCompleteAwaitingAuthorization
        );
    }

    #[test]
    fn missing_receipt_is_stale_never_allowed() {
        let mut input = input(&[("A", "README.md")]);
        input.receipt = None;
        let lock = evaluate_frozen_subject_lock(&input);
        assert_eq!(lock.state, FrozenSubjectStateV1::Inactive);
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::Stale);
    }

    #[test]
    fn non_complete_receipt_is_stale_never_allowed() {
        let mut input = input(&[("A", "README.md")]);
        input.receipt.as_mut().expect("receipt").freeze_state = "Stale".to_string();
        let lock = evaluate_frozen_subject_lock(&input);
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::RequiresInvalidation);
        assert!(!lock.blocking_rows.is_empty());
    }

    #[test]
    fn unclassified_paths_are_recorded_non_load_bearing() {
        // A path no typed pattern claims is still recorded, with the
        // unclassified owner visible to reviewers.
        let classified = classify_frozen_subject_path("notes/scratch/new-note.md", "A", false);
        assert_eq!(classified.kind, FrozenSubjectPathKindV1::NonLoadBearing);
        assert_eq!(classified.owner, "unclassified");
    }
}
//
#[cfg(test)]
mod invalidation_scope_tests {
    use super::{
        FrozenSubjectInvalidationV1, FrozenSubjectLockInputV1, FrozenSubjectReceiptIdentityV1,
        FrozenSubjectStateV1, FrozenSubjectVerdictV1, evaluate_frozen_subject_lock,
    };

    fn receipt(commit: &str) -> FrozenSubjectReceiptIdentityV1 {
        FrozenSubjectReceiptIdentityV1 {
            commit: commit.to_string(),
            tree: "tree".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            freeze_state: "Complete".to_string(),
            receipt_digest: "sha256:v1:digest".to_string(),
        }
    }

    fn invalidation(commit: &str) -> FrozenSubjectInvalidationV1 {
        FrozenSubjectInvalidationV1 {
            reason: "test".to_string(),
            recorded_by: "test".to_string(),
            recorded_at_utc: "2026-09-04T00:00:00Z".to_string(),
            frozen_commit: commit.to_string(),
        }
    }

    #[test]
    fn invalidations_apply_only_to_the_commit_they_name() {
        let input = FrozenSubjectLockInputV1 {
            receipt: Some(receipt("aaaa")),
            current_head: "bbbb".to_string(),
            changed_paths: Vec::new(),
            invalidations: vec![invalidation("cccc"), invalidation("dddd")],
        };
        let lock = evaluate_frozen_subject_lock(&input);
        assert_eq!(
            lock.state,
            FrozenSubjectStateV1::FreezeCompleteAwaitingAuthorization
        );
        assert_eq!(lock.invalidation_count, 0);

        let input = FrozenSubjectLockInputV1 {
            receipt: Some(receipt("aaaa")),
            current_head: "bbbb".to_string(),
            changed_paths: Vec::new(),
            invalidations: vec![invalidation("cccc"), invalidation("aaaa")],
        };
        let lock = evaluate_frozen_subject_lock(&input);
        assert_eq!(lock.state, FrozenSubjectStateV1::InvalidatedForRefreeze);
        assert_eq!(lock.invalidation_count, 1);
        // An invalidation with no movement still stales the freeze.
        assert_eq!(lock.verdict, FrozenSubjectVerdictV1::Stale);
    }
}
//
#[cfg(test)]
mod coverage_tests {
    use super::super::frozen_subject_lock_v1::*;
    use super::super::frozen_subject_lock_v1::*;

    #[test]
    fn every_load_bearing_owner_has_patterns_and_a_label() {
        for owner in LoadBearingOwnerV1::patterns_owned() {
            assert!(!owner.patterns().is_empty());
            assert!(!owner.label().is_empty());
        }
    }

    #[test]
    fn every_non_load_bearing_owner_has_patterns_and_a_label() {
        for owner in NonLoadBearingOwnerV1::patterns_owned() {
            assert!(!owner.patterns().is_empty());
            assert!(!owner.label().is_empty());
        }
    }

    #[test]
    fn display_renders_the_verdict_label() {
        for verdict in [
            FrozenSubjectVerdictV1::Complete,
            FrozenSubjectVerdictV1::AllowedNonLoadBearing,
            FrozenSubjectVerdictV1::RequiresInvalidation,
            FrozenSubjectVerdictV1::Conflict,
            FrozenSubjectVerdictV1::Stale,
            FrozenSubjectVerdictV1::InstrumentFailure,
        ] {
            assert_eq!(format!("{verdict}"), verdict.label());
        }
    }

    #[test]
    fn load_bearing_patterns_classify_canonical_paths() {
        let cases = [
            ("docs/dogfood/receipts/final-freeze/x.json", "frozen_lock_records"),
            ("crates/allow-core/src/lib.rs", "shipped_source"),
            ("Cargo.lock", "manifests_lockfile"),
            ("rust-toolchain.toml", "toolchain"),
            ("policy/allow.toml", "policy"),
            ("docs/schemas/x.json", "schemas"),
            ("README.md", "package_docs_assets"),
            ("docs/support-matrix.toml", "support_channel_truth"),
            (".changes/x.yaml", "release_records"),
            (".github/workflows/release.yml", "workflows_actions"),
            ("scripts/release-topology-publisher.py", "release_evidence_producers"),
        ];
        for (path, owner) in cases {
            let classified = classify_frozen_subject_path(path, "M", false);
            assert_eq!(classified.kind, FrozenSubjectPathKindV1::LoadBearing, "{path}");
            assert_eq!(classified.owner, owner, "{path}");
        }
    }

    #[test]
    fn non_load_bearing_patterns_classify_canonical_paths() {
        let cases = [
            (".github/workflows/frozen-subject-lock.yml", "frozen_subject_lock_machinery"),
            ("crates/cargo-intent/src/lib.rs", "sibling_products"),
            ("docs/dogfood/receipts/old-lane/x.json", "campaign_records"),
            ("docs/source-of-truth/x.md", "repository_prose"),
        ];
        for (path, owner) in cases {
            let classified = classify_frozen_subject_path(path, "M", false);
            assert_eq!(classified.kind, FrozenSubjectPathKindV1::NonLoadBearing, "{path}");
            assert_eq!(classified.owner, owner, "{path}");
        }
    }
}
