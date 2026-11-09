//! Drift guard over the retained direct minimum version proofs
//! (#3903 PR D): a live-tree observation of one product's selection is
//! graded against the retained receipt so dependency, feature, target,
//! MSRV, package-set, or manifest movement reruns or stales the
//! affected request instead of silently passing on old evidence.
//!
//! Laws:
//! - a missing or wrong-product receipt is `unproven`, never current;
//! - MSRV, manifest-set, lock, or package-roots movement is `stale`
//!   with the exact movement named;
//! - a receipt execution-provenance row (#4214) whose selected-source
//!   projection digest moved against the observation's digest is
//!   `stale` even when the manifest-set and lock digests are equal: a
//!   Rust-source-only edit cannot inherit the earlier proof. A receipt
//!   predating the section stays decodable historical evidence graded
//!   on the fields it carries, and a present-but-invalid provenance
//!   row (blank identity fields, non-canonical digests, an executable
//!   path without its digest) is a named `incomplete` failure;
//! - coverage is graded only against an identity-current receipt: a
//!   declared floor without a receipt row and a receipt row that is no
//!   longer declared are both named;
//! - unsupported, resolver-failed, and instrument-failed rows stay
//!   visible as `incomplete` rather than defaulting to current-lock
//!   success;
//! - only the cargo-allow release set blocks; advisory product drift
//!   reports the same verdicts without blocking (negative control 10).

use serde::{Deserialize, Serialize};

use crate::artifacts::minimum_direct_version_v1::{
    MINIMUM_DIRECT_VERSION_SCHEMA_ID, MINIMUM_DIRECT_VERSION_SCHEMA_VERSION, MinimumFloorResultV1,
    MinimumVersionExecutionProvenanceV1, MinimumVersionProofReceiptV1, target_matches_selection,
    toolchain_matches_msrv,
};

pub const MINIMUM_DIRECT_VERSION_DRIFT_SCHEMA_ID: &str =
    "cargo-allow.minimum-direct-version-drift.v1";
pub const MINIMUM_DIRECT_VERSION_DRIFT_SCHEMA_VERSION: u32 = 1;

/// The one product whose stale or missing proof blocks; every other
/// product stays advisory regardless of its drift verdict.
pub const DRIFT_RELEASE_SET_PRODUCT: &str = "cargo-allow";

const DRIFT_CLAIM_BOUNDARY: &str = "Drift guard over retained direct-floor proofs: identity movement stales the affected request, coverage and non-clean rows stay visible, and only cargo-allow release-set drift blocks. Transitive minimal graphs are not evaluated.";

/// One declared external direct dependency floor of the live-tree
/// selection: the same three identity fields the proof receipt's rows
/// carry, without any proof outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionDriftFloorRowV1 {
    pub package: String,
    pub declared_requirement: String,
    pub selected_floor: String,
}

/// The live-tree observation of one product's direct-floor selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionDriftObservationV1 {
    pub product: String,
    pub package_roots: Vec<String>,
    pub msrv: String,
    pub target: String,
    pub manifest_set_digest: String,
    pub lock_digest: String,
    /// The live selected-source projection digest (#4214), derived the
    /// same way the producer derives the receipt's provenance digest.
    /// `None` when the caller cannot derive it; a receipt provenance
    /// row is then not source-graded rather than silently passed.
    #[serde(default)]
    pub selected_source_digest: Option<String>,
    pub floors: Vec<MinimumVersionDriftFloorRowV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimumVersionDriftVerdictV1 {
    Current,
    Stale,
    Unproven,
    Incomplete,
}

impl MinimumVersionDriftVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Unproven => "unproven",
            Self::Incomplete => "incomplete",
        }
    }
}

/// The graded drift state of one product's retained proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionDriftEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub product: String,
    pub verdict: MinimumVersionDriftVerdictV1,
    pub reasons: Vec<String>,
    /// Only the release set's non-current drift blocks; advisory
    /// products report the same verdicts without blocking.
    pub blocking: bool,
    pub claim_boundary: String,
}

/// Grade one product's live-tree observation against its retained
/// receipt. Pure and fail-closed: every movement is named, and only a
/// fully aligned, all-Proven, fully covering receipt is `current`.
#[must_use]
pub fn evaluate_minimum_version_drift(
    observation: &MinimumVersionDriftObservationV1,
    receipt: Option<&MinimumVersionProofReceiptV1>,
) -> MinimumVersionDriftEvaluationV1 {
    let finish = |verdict: MinimumVersionDriftVerdictV1, reasons: Vec<String>| {
        MinimumVersionDriftEvaluationV1 {
            schema_id: MINIMUM_DIRECT_VERSION_DRIFT_SCHEMA_ID.to_string(),
            schema_version: MINIMUM_DIRECT_VERSION_DRIFT_SCHEMA_VERSION,
            product: observation.product.clone(),
            verdict,
            blocking: observation.product == DRIFT_RELEASE_SET_PRODUCT
                && verdict != MinimumVersionDriftVerdictV1::Current,
            reasons,
            claim_boundary: DRIFT_CLAIM_BOUNDARY.to_string(),
        }
    };

    let mut reasons = Vec::new();

    let Some(receipt) = receipt else {
        reasons.push(format!(
            "no retained receipt for product {}",
            observation.product
        ));
        return finish(MinimumVersionDriftVerdictV1::Unproven, reasons);
    };
    if receipt.product != observation.product {
        reasons.push(format!(
            "receipt names product {}, not {}",
            receipt.product, observation.product
        ));
        return finish(MinimumVersionDriftVerdictV1::Unproven, reasons);
    }

    // Negative control 8: source/manifests/features/MSRV movement
    // invalidates the evidence instead of defaulting to it.
    if receipt.msrv != observation.msrv {
        reasons.push(format!(
            "msrv moved: receipt {} vs current {}",
            receipt.msrv, observation.msrv
        ));
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }
    if receipt.manifest_set_digest != observation.manifest_set_digest {
        reasons.push("the checked-in manifests moved since the receipt".to_string());
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }
    if receipt.lock_digest != observation.lock_digest {
        reasons.push("Cargo.lock moved since the receipt".to_string());
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }
    // #4214: a present execution provenance binds the receipt to the
    // executed selected-source projection. When the manifest and lock
    // digests above are equal, a projection mismatch still stales the
    // receipt — a Rust-source-only edit (for example, a new dependency
    // API use above the recorded floor) must not inherit the earlier
    // proof. A receipt predating the section carries no projection
    // binding and stays graded on the fields it recorded; the missing
    // section cannot manufacture missing current-source proof, and no
    // source movement is invented for it.
    if let (Some(provenance), Some(current_source)) = (
        &receipt.execution_provenance,
        &observation.selected_source_digest,
    ) && provenance.selected_source_digest != *current_source
    {
        reasons.push(format!(
            "selected source moved since the receipt: receipt projection {} vs current {} \
             (manifests and Cargo.lock are unchanged; the selected Rust source inputs moved)",
            provenance.selected_source_digest, current_source
        ));
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }
    // Receipts that predate root recording carry no roots binding; the
    // observation's roots then govern the coverage grading alone.
    if !receipt.package_roots.is_empty() && receipt.package_roots != observation.package_roots {
        reasons.push(format!(
            "package roots moved: receipt {:?} vs current {:?}",
            receipt.package_roots, observation.package_roots
        ));
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }

    // Identity-current: grade the receipt's full proof contract, not
    // merely package-name coverage — schema identity, toolchain,
    // execution evidence, complete floor-row identity, and non-clean
    // dispositions all stay visible.
    if observation.floors.is_empty() {
        reasons.push("empty floor denominator: no floor rows were selected".to_string());
    }
    if receipt.schema_id != MINIMUM_DIRECT_VERSION_SCHEMA_ID {
        reasons.push(format!(
            "receipt schema {} is not {}",
            receipt.schema_id, MINIMUM_DIRECT_VERSION_SCHEMA_ID
        ));
    }
    if receipt.schema_version != MINIMUM_DIRECT_VERSION_SCHEMA_VERSION {
        reasons.push(format!(
            "receipt schema version {} is not {}",
            receipt.schema_version, MINIMUM_DIRECT_VERSION_SCHEMA_VERSION
        ));
    }
    if !toolchain_matches_msrv(&receipt.toolchain, &observation.msrv) {
        reasons.push(format!(
            "receipt toolchain {} does not match the claimed msrv {}",
            receipt.toolchain, observation.msrv
        ));
    }
    if !target_matches_selection(&receipt.target, &observation.target) {
        reasons.push(format!(
            "target moved: receipt {} vs current {}",
            receipt.target, observation.target
        ));
    }
    if receipt.commands.is_empty() {
        reasons.push("receipt records no executed commands".to_string());
    }
    // #4214/#4225: a present-but-invalid provenance row is a named
    // failure, never a silent pass. A missing row stays historical.
    if let Some(provenance) = &receipt.execution_provenance {
        reasons.extend(execution_provenance_shape_failures(provenance));
    }
    for floor in &observation.floors {
        let Some(row) = receipt.rows.iter().find(|row| row.package == floor.package) else {
            reasons.push(format!(
                "no receipt row for declared floor {}",
                floor.package
            ));
            continue;
        };
        if row.declared_requirement != floor.declared_requirement {
            reasons.push(format!(
                "declared requirement moved for {}: receipt {} vs current {}",
                floor.package, row.declared_requirement, floor.declared_requirement
            ));
        }
        if row.tested_floor != floor.selected_floor {
            reasons.push(format!(
                "tested floor moved for {}: receipt {} vs current {}",
                floor.package, row.tested_floor, floor.selected_floor
            ));
        }
        // A proven row must have resolved exactly the tested floor; a
        // compatible newer version proves nothing about the floor.
        // Registry build metadata (+spec-1.1.0) carries no SemVer
        // precedence, so the comparison uses the base version.
        let resolved_base = row
            .resolved_version
            .split('+')
            .next()
            .unwrap_or(&row.resolved_version);
        if row.result == MinimumFloorResultV1::Proven && resolved_base != row.tested_floor {
            reasons.push(format!(
                "resolved substitution for {}: resolved {} but the tested floor is {}",
                floor.package, row.resolved_version, row.tested_floor
            ));
        }
    }
    for row in &receipt.rows {
        if !observation
            .floors
            .iter()
            .any(|floor| floor.package == row.package)
        {
            reasons.push(format!(
                "receipt row {} is no longer a declared floor",
                row.package
            ));
        }
    }
    let unclean: Vec<&str> = receipt
        .rows
        .iter()
        .filter(|row| row.result != MinimumFloorResultV1::Proven)
        .map(|row| row.package.as_str())
        .collect();
    if !unclean.is_empty() {
        reasons.push(format!("non-clean dispositions: {}", unclean.join(", ")));
    }

    if reasons.is_empty() {
        finish(MinimumVersionDriftVerdictV1::Current, reasons)
    } else {
        finish(MinimumVersionDriftVerdictV1::Incomplete, reasons)
    }
}

/// The #4214/#4225 provenance shape law: a present provenance row must
/// be internally valid — identity fields non-empty, the selected-source
/// digest bare sha256 hex, and an executable path never recorded
/// without its canonical `sha256:v1:` digest. A malformed row is a
/// named failure, never a silent pass; a missing row stays historical.
fn execution_provenance_shape_failures(
    provenance: &MinimumVersionExecutionProvenanceV1,
) -> Vec<String> {
    let mut failures = Vec::new();
    for (field, value) in [
        ("command", &provenance.command),
        ("working directory", &provenance.working_directory),
        ("source commit", &provenance.source_commit),
        ("source tree", &provenance.source_tree),
    ] {
        if value.trim().is_empty() {
            failures.push(format!(
                "execution provenance is malformed: the {field} is empty"
            ));
        }
    }
    if !canonical_selected_source_digest(&provenance.selected_source_digest) {
        failures.push(
            "execution provenance is malformed: the selected-source digest is not bare sha256 hex"
                .to_string(),
        );
    }
    match (&provenance.executable_path, &provenance.executable_digest) {
        (Some(path), Some(digest)) => {
            if path.trim().is_empty() {
                failures.push(
                    "execution provenance is malformed: the executable path is empty".to_string(),
                );
            }
            if !canonical_executable_digest(digest) {
                failures.push(
                    "execution provenance is malformed: the executable digest is not canonical sha256:v1: hex"
                        .to_string(),
                );
            }
        }
        (Some(_), None) => failures.push(
            "execution provenance is malformed: an executable path is recorded without its digest"
                .to_string(),
        ),
        (None, Some(_)) => failures.push(
            "execution provenance is malformed: an executable digest is recorded without its path"
                .to_string(),
        ),
        (None, None) => {}
    }
    failures
}

/// The selected-source projection digest is bare sha256 hex: 64 hex
/// digits with no prefix, matching the receipt's manifest and lock
/// digest encoding.
fn canonical_selected_source_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The executable digest follows the shared byte-identity law
/// (`sha256:v1:` plus 64 hex digits) the tool-identity contract uses.
fn canonical_executable_digest(text: &str) -> bool {
    text.strip_prefix("sha256:v1:")
        .is_some_and(canonical_selected_source_digest)
}

/// Human view of one drift evaluation.
#[must_use]
pub fn render_minimum_version_drift_human(evaluation: &MinimumVersionDriftEvaluationV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "minimum-version-drift: product={} verdict={} blocking={}",
        evaluation.product,
        evaluation.verdict.label(),
        evaluation.blocking
    ));
    for reason in &evaluation.reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!("  claim boundary: {}", evaluation.claim_boundary));
    lines.join("\n")
}

/// JSON view of one drift evaluation.
///
/// # Errors
///
/// Returns the serialization error when the evaluation cannot be
/// rendered as JSON.
pub fn render_minimum_version_drift_json(
    evaluation: &MinimumVersionDriftEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}
