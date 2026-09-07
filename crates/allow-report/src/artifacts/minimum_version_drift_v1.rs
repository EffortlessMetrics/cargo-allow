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
    MinimumFloorResultV1, MinimumVersionProofReceiptV1,
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
    pub manifest_set_digest: String,
    pub lock_digest: String,
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
    // Receipts that predate root recording carry no roots binding; the
    // observation's roots then govern the coverage grading alone.
    if !receipt.package_roots.is_empty() && receipt.package_roots != observation.package_roots {
        reasons.push(format!(
            "package roots moved: receipt {:?} vs current {:?}",
            receipt.package_roots, observation.package_roots
        ));
        return finish(MinimumVersionDriftVerdictV1::Stale, reasons);
    }

    // Identity-current: grade floor coverage in both directions.
    for floor in &observation.floors {
        if !receipt.rows.iter().any(|row| row.package == floor.package) {
            reasons.push(format!(
                "no receipt row for declared floor {}",
                floor.package
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
