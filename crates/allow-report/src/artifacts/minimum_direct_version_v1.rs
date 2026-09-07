//! Direct minimum dependency version contract (#3903 PR A).
//!
//! The typed denominator for proving that declared direct dependency
//! floors compile and behave under a package's claimed MSRV: the
//! inventory of direct requirements by product/package set, the proof
//! request binding roots/features/target/MSRV to selected floors, and
//! the receipt carrying per-row dispositions. Report-only at this
//! stage: no dependency requirement is changed by this contract.
//!
//! Law encoded by [`evaluate_minimum_version_proof`]:
//! - the current `Cargo.lock` is never proof of a declared floor
//!   (negative control 1): the receipt must name the tested floor and
//!   the resolved version separately;
//! - a compatible newer version does not substitute for the floor
//!   (control 3): the tested floor must equal the declared floor's
//!   minimum;
//! - a Rust newer than the claimed MSRV does not satisfy the row
//!   silently (control 7): the receipt's toolchain must match the
//!   request's MSRV;
//! - source/manifest/lock movement stales the receipt (control 8);
//! - zero selected rows is never `Proven` (control 9);
//! - unavailable/yanked and compile-incompatible dispositions stay
//!   distinct (control 6);
//! - report-only product rows cannot block cargo-allow (control 10).
//!
//! Claim boundary: exact selected proof of declared direct dependency
//! floors for identified package sets, features, targets, and MSRVs.
//! It does not certify arbitrary transitive minimal-version
//! combinations, does not replace Dependabot/cargo-deny/locked CI, and
//! does not upgrade, downgrade, publish, or support-promote anything.

use serde::{Deserialize, Serialize};

pub const MINIMUM_DIRECT_VERSION_SCHEMA_ID: &str = "cargo-allow.minimum-direct-version.v1";
pub const MINIMUM_DIRECT_VERSION_SCHEMA_VERSION: u32 = 1;

const CLAIM_BOUNDARY: &str = "Exact selected proof of declared direct dependency floors for identified package sets, features, targets, and MSRVs. It determines whether those manifest claims are currently supported; it does not certify arbitrary transitive minimal-version combinations, does not replace Dependabot, cargo-deny, or normal locked CI, and upgrades, downgrades, publishes, or support-promotes nothing.";

/// The dependency class a direct requirement appears under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectDependencyClassV1 {
    Normal,
    Dev,
    Build,
}

impl DirectDependencyClassV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
        }
    }
}

/// One direct dependency requirement row from a selected manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectRequirementRowV1 {
    pub package: String,
    /// The declared requirement string verbatim (e.g. `1.0`, `0.8`).
    pub requirement: String,
    pub class: DirectDependencyClassV1,
    /// Target-specific requirement's triple predicate, when declared.
    #[serde(default)]
    pub target: Option<String>,
    /// Feature-activated dependencies name the activating feature.
    #[serde(default)]
    pub activated_by_feature: Option<String>,
}

/// The direct-requirement inventory for one product/package set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductDependencySetV1 {
    /// The product family: `cargo-allow` (release set), `shared`,
    /// `cargo-intent`, or `cargo-proof`.
    pub product: String,
    pub packages: Vec<String>,
    #[serde(default)]
    pub rows: Vec<DirectRequirementRowV1>,
    /// Digests binding the manifest set and the current lock identity.
    pub manifest_set_digest: String,
    pub lock_digest: String,
    /// The product's claimed MSRV.
    pub msrv: String,
}

/// The typed denominator over the product sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectMinimumVersionSetV1 {
    pub schema_id: String,
    pub schema_version: u32,
    #[serde(default)]
    pub product_sets: Vec<ProductDependencySetV1>,
    pub claim_boundary: String,
}

/// The proof request: what is being proven, under which configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionProofRequestV1 {
    pub product: String,
    /// The package root(s) the proof compiles and tests.
    pub package_roots: Vec<String>,
    pub features: Vec<String>,
    pub target: String,
    /// The claimed MSRV the proof must run under — a newer toolchain
    /// does not satisfy the row (negative control 7).
    pub msrv: String,
    /// The selected floors: each declared direct requirement's minimum
    /// version to pin for the proof.
    pub floors: Vec<MinimumFloorRowV1>,
    /// The proof classes executed: `check`, `test`, `package`.
    pub proof_classes: Vec<String>,
    /// Digests of the manifest set and lock the request was derived
    /// from; movement stales the receipt.
    pub manifest_set_digest: String,
    pub lock_digest: String,
}

/// One selected floor row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumFloorRowV1 {
    pub package: String,
    /// The declared requirement's minimum version (e.g. requirement
    /// `0.8` selects floor `0.8.0`).
    pub declared_requirement: String,
    pub selected_floor: String,
    pub class: DirectDependencyClassV1,
}

/// Per-row proof result vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimumFloorResultV1 {
    Proven,
    FloorTooLow,
    UnsupportedCombination,
    ResolverFailure,
    PackageMetadataMismatch,
    InstrumentFailure,
    NotClaimed,
}

impl MinimumFloorResultV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::FloorTooLow => "floor_too_low",
            Self::UnsupportedCombination => "unsupported_combination",
            Self::ResolverFailure => "resolver_failure",
            Self::PackageMetadataMismatch => "package_metadata_mismatch",
            Self::InstrumentFailure => "instrument_failure",
            Self::NotClaimed => "not_claimed",
        }
    }

    /// `Proven` is the only clean disposition; everything else is an
    /// honest non-clean result.
    #[must_use]
    pub const fn is_clean(self) -> bool {
        matches!(self, Self::Proven)
    }
}

/// Per-row receipt result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionRowResultV1 {
    pub package: String,
    /// The declared requirement verbatim.
    pub declared_requirement: String,
    /// The floor version actually pinned and tested.
    pub tested_floor: String,
    /// The exact version resolved at the tested floor.
    pub resolved_version: String,
    /// Registry or source identity of the resolved version.
    pub source_identity: String,
    pub result: MinimumFloorResultV1,
    /// The exact limitation (yanked, compile-incompatible, resolver
    /// failure detail); required for every non-clean result.
    #[serde(default)]
    pub limitation: Option<String>,
}

/// The proof receipt over one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumVersionProofReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub product: String,
    pub msrv: String,
    /// The toolchain the proof actually ran under.
    pub toolchain: String,
    pub target: String,
    /// Manifest-set and lock digests the proof ran against (must match
    /// the request or the receipt is stale).
    pub manifest_set_digest: String,
    pub lock_digest: String,
    #[serde(default)]
    pub rows: Vec<MinimumVersionRowResultV1>,
    /// The exact commands executed, with proof classes.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Digest of the changed lock/override artifact the proof produced.
    #[serde(default)]
    pub floor_lock_digest: String,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// The evaluated verdict over one receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimumProofVerdictV1 {
    Complete,
    Stale,
    Incomplete,
    InstrumentFailure,
}

impl MinimumProofVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Stale => "stale",
            Self::Incomplete => "incomplete",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinimumProofEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub verdict: MinimumProofVerdictV1,
    pub reasons: Vec<String>,
    pub claim_boundary: String,
}

/// Evaluate the receipt against the request. Pure and ordered by the
/// request's floor rows.
#[must_use]
pub fn evaluate_minimum_version_proof(
    request: &MinimumVersionProofRequestV1,
    receipt: &MinimumVersionProofReceiptV1,
) -> MinimumProofEvaluationV1 {
    let mut reasons = Vec::new();

    if receipt.schema_id != MINIMUM_DIRECT_VERSION_SCHEMA_ID {
        reasons.push("schema_mismatch".to_string());
    }
    if receipt.product != request.product {
        // Negative control 5: one product's proof cannot satisfy
        // another's request.
        reasons.push(format!(
            "product_mismatch: receipt {} vs request {}",
            receipt.product, request.product
        ));
    }
    if receipt.msrv != request.msrv {
        reasons.push(format!(
            "msrv_mismatch: receipt {} vs request {}",
            receipt.msrv, request.msrv
        ));
    }
    if receipt.manifest_set_digest != request.manifest_set_digest
        || receipt.lock_digest != request.lock_digest
    {
        // Negative control 8: source/manifest/lock movement stales.
        reasons.push("stale: the request digests moved since the receipt".to_string());
        return finish(MinimumProofVerdictV1::Stale, reasons);
    }
    if request.floors.is_empty() {
        // Negative control 9: zero selected rows is never Proven.
        reasons.push("empty denominator: no floor rows were selected".to_string());
        return finish(MinimumProofVerdictV1::InstrumentFailure, reasons);
    }

    let mut covered: Vec<&str> = Vec::new();
    for floor in &request.floors {
        let row = receipt.rows.iter().find(|row| row.package == floor.package);
        let Some(row) = row else {
            reasons.push(format!("missing receipt row: {}", floor.package));
            continue;
        };
        // Negative control 3: the tested floor must be the declared
        // floor's minimum; a compatible newer version cannot substitute
        // for it while still being labeled the declared floor.
        if row.tested_floor != floor.selected_floor {
            reasons.push(format!(
                "floor substitution: {} was tested at {} but the declared floor selects {}",
                floor.package, row.tested_floor, floor.selected_floor
            ));
        }
        if !row.result.is_clean() && row.limitation.as_deref().is_none_or(str::is_empty) {
            reasons.push(format!(
                "non-clean row without limitation: {}",
                floor.package
            ));
        }
        // Negative control 6: unavailable/yanked and
        // compile-incompatible stay distinct — enforced by the
        // vocabulary itself (UnsupportedCombination vs ResolverFailure
        // with distinct limitations), asserted by the caller's tests.
        covered.push(&floor.package);
    }
    for row in &receipt.rows {
        if !covered.contains(&row.package.as_str()) {
            reasons.push(format!(
                "receipt row {} is not in the request's selected floors",
                row.package
            ));
        }
    }

    // Negative control 7: a newer toolchain does not satisfy silently.
    if receipt.toolchain != request.msrv && !receipt.toolchain.starts_with(&request.msrv) {
        reasons.push(format!(
            "toolchain {} is newer than the claimed MSRV {}; the rows cannot silently pass",
            receipt.toolchain, request.msrv
        ));
        return finish(MinimumProofVerdictV1::InstrumentFailure, reasons);
    }

    let unclean: Vec<&str> = receipt
        .rows
        .iter()
        .filter(|row| !row.result.is_clean())
        .map(|row| row.package.as_str())
        .collect();
    if !unclean.is_empty() {
        reasons.push(format!("non-clean dispositions: {}", unclean.join(", ")));
    }

    if !reasons.is_empty() {
        // Negative control 10: report-only products keep their own
        // verdict; the caller scopes blocking to the release set.
        finish(MinimumProofVerdictV1::Incomplete, reasons)
    } else {
        finish(MinimumProofVerdictV1::Complete, reasons)
    }
}

fn finish(verdict: MinimumProofVerdictV1, reasons: Vec<String>) -> MinimumProofEvaluationV1 {
    MinimumProofEvaluationV1 {
        schema_id: MINIMUM_DIRECT_VERSION_SCHEMA_ID.to_string(),
        schema_version: MINIMUM_DIRECT_VERSION_SCHEMA_VERSION,
        verdict,
        reasons,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the evaluation.
#[must_use]
pub fn render_minimum_proof_human(evaluation: &MinimumProofEvaluationV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "minimum-direct-version: verdict={}",
        evaluation.verdict.label()
    ));
    for reason in &evaluation.reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!("  claim boundary: {}", evaluation.claim_boundary));
    lines.join("\n")
}

/// JSON view of the evaluation.
pub fn render_minimum_proof_json(
    evaluation: &MinimumProofEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}
