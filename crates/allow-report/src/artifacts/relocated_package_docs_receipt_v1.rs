//! Relocated package-documentation receipt (#3852).
//!
//! Proves, from the exact final `.crate` bytes (never the source tree),
//! that each of the ten cargo-allow-family packages documents, builds its
//! docs, runs its doctests, compile-checks its examples, binds its
//! README/license assets, and keeps its role/claim boundary while relocated
//! outside the repository with crates.io replaced by a local registry.
//!
//! `Complete` means the relocated package-documentation proof for that row
//! is whole. It does not mean public docs.rs rendering, install-channel
//! availability, authorization, or release publication are complete.

use serde::{Deserialize, Serialize};

pub const RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1: &str = "cargo-allow.relocated-package-docs.v1";

pub const RELOCATED_PACKAGE_DOCS_CLAIM_BOUNDARY_V1: &str = "Relocated proof that the exact final .crate files document, build docs, run doctests, compile-check examples, bind README/license assets, and keep role/claim boundaries outside the repository against a local registry. It proves shipped package documentation works relocated; it does not prove public docs.rs rendering, install-channel availability, authorization, or release publication.";

pub const RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1: &[&str] = &[
    "allow-core",
    "allow-policy",
    "allow-inventory",
    "allow-files",
    "allow-rust",
    "allow-match",
    "allow-report",
    "allow-policy-legacy",
    "allow-diff",
    "cargo-allow",
];

/// Terminal result of one relocated package-documentation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocatedPackageDocsResultV1 {
    Complete,
    Incomplete,
    Stale,
    Mismatch,
    Unsupported,
    InstrumentFailure,
}

/// Consumed-input basis the receipt reconciles against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedPackageDocsBasisV1 {
    /// sha256 of the ExactCandidatePackageSetV1 receipt bytes consumed.
    pub exact_candidate_receipt_digest: String,
    /// sha256 of the final-packaged-surface (#3851) receipt bytes consumed.
    pub packaged_surface_digest: String,
    /// Git head the exact `.crate` files were packaged from.
    pub git_head: String,
    /// Toolchain that executed the relocated runs (e.g. `1.98.1`).
    pub toolchain: String,
    /// Honest network posture, e.g. `fetch_warm_may_use_crates_io`.
    pub network_posture: String,
    /// Isolation mechanism, e.g. `local_registry_offline`.
    pub isolation: String,
}

/// Channel/support projection bound from the canonical repo source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedPackageDocsReferenceProjectionV1 {
    pub published_version: String,
    pub candidate_version: String,
    /// sha256 over the canonical `docs/support-matrix.toml` and
    /// `docs/getting-started.md` bytes this row was projected from.
    pub channel_digest: String,
}

/// One relocated package-documentation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedPackageDocsRowV1 {
    pub name: String,
    pub version: String,
    /// Archive sha256 (hex) reconciled against the #3851 surface row.
    pub crate_sha256: String,
    pub crate_size_bytes: u64,
    /// sha256 (hex) of the normalized packaged `Cargo.toml`.
    pub manifest_digest: String,
    /// sha256 (hex) of the packaged file list.
    pub file_list_digest: String,
    pub readme_present: bool,
    pub readme_sha256: Option<String>,
    pub license_assets_present: bool,
    /// Repo-relative prose references (e.g. `docs/claim-boundaries.md`)
    /// that resolve to a real file in the canonical repo source and point
    /// outside the archive: bounded postures, never silently passed.
    pub external_repo_refs: Vec<String>,
    pub reference_projection: RelocatedPackageDocsReferenceProjectionV1,
    pub check_exit: i32,
    pub check_warnings: u32,
    pub doc_exit: i32,
    pub doc_warnings: u32,
    /// rustdoc posture: `clean` means exit 0 with zero warnings.
    pub doc_posture: String,
    pub doctest_exit: i32,
    pub doctests_passed: u32,
    /// `has_doctests` or `no_public_doctests`.
    pub doctest_posture: String,
    /// `examples_built` or the explicit `NoExampleSelected` posture.
    pub examples_posture: String,
    pub examples_exit: Option<i32>,
    /// Selected/default feature set from the packaged manifest.
    pub features: Vec<String>,
    pub features_posture: String,
    /// Exit of the extra `--no-default-features` check, run only where the
    /// package publicly documents the feature-disabled configuration.
    pub no_default_check_exit: Option<i32>,
    /// Checked role/limitation markers observed in the packaged docs.
    pub role_marker: String,
    pub limitation_marker: String,
    pub sibling_products_separate: bool,
    pub result: RelocatedPackageDocsResultV1,
    pub limitations: Vec<String>,
}

/// One encoded negative control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedPackageDocsNegativeV1 {
    pub id: String,
    pub result_class: String,
    pub detail: String,
}

/// Aggregate outcome over the ten rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocatedPackageDocsAggregateV1 {
    pub result: RelocatedPackageDocsResultV1,
    pub complete: usize,
    pub incomplete: usize,
}

/// The final typed relocated package-docs receipt (#3852).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowRelocatedPackageDocsReceiptV1 {
    pub schema: String,
    pub basis: RelocatedPackageDocsBasisV1,
    pub rows: Vec<RelocatedPackageDocsRowV1>,
    pub aggregate: RelocatedPackageDocsAggregateV1,
    pub negative_controls: Vec<RelocatedPackageDocsNegativeV1>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

impl CargoAllowRelocatedPackageDocsReceiptV1 {
    /// Start a receipt with the fixed framing fields.
    pub fn new(basis: RelocatedPackageDocsBasisV1) -> Self {
        Self {
            schema: RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1.to_string(),
            basis,
            rows: Vec::new(),
            aggregate: RelocatedPackageDocsAggregateV1 {
                result: RelocatedPackageDocsResultV1::InstrumentFailure,
                complete: 0,
                incomplete: 0,
            },
            negative_controls: Vec::new(),
            claim_boundary: RELOCATED_PACKAGE_DOCS_CLAIM_BOUNDARY_V1.to_string(),
            limitations: Vec::new(),
        }
    }

    /// Recompute the aggregate from the rows.
    pub fn refresh_aggregate(&mut self) {
        let complete = self
            .rows
            .iter()
            .filter(|row| row.result == RelocatedPackageDocsResultV1::Complete)
            .count();
        let incomplete = self.rows.len().saturating_sub(complete);
        let result = if self.rows.len() == RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.len()
            && incomplete == 0
        {
            RelocatedPackageDocsResultV1::Complete
        } else if incomplete > 0 {
            RelocatedPackageDocsResultV1::Incomplete
        } else {
            RelocatedPackageDocsResultV1::InstrumentFailure
        };
        self.aggregate = RelocatedPackageDocsAggregateV1 {
            result,
            complete,
            incomplete,
        };
    }

    /// Validate receipt shape: schema id, ten expected rows, stable
    /// versions (rc-line inputs rejected as final identity), declared
    /// assets present on every Complete row, and an aggregate (counts and
    /// result) coherent with the rows.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1 {
            return Err(format!(
                "schema must be {RELOCATED_PACKAGE_DOCS_RECEIPT_SCHEMA_V1}"
            ));
        }
        if self.rows.len() != RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.len() {
            return Err(format!(
                "expected {} rows, found {}",
                RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.len(),
                self.rows.len()
            ));
        }
        for (idx, expected) in RELOCATED_PACKAGE_DOCS_EXPECTED_ROWS_V1.iter().enumerate() {
            if self.rows.get(idx).map(|row| row.name.as_str()) != Some(*expected) {
                return Err(format!("row {idx} must be {expected} in release order"));
            }
        }
        for row in &self.rows {
            if row.version.contains('-') {
                return Err(format!(
                    "row {} version {:?} is an rc-line input, rejected as final identity",
                    row.name, row.version
                ));
            }
            if row.result == RelocatedPackageDocsResultV1::Complete {
                if !row.readme_present {
                    return Err(format!(
                        "row {} is Complete but its declared readme asset is absent",
                        row.name
                    ));
                }
                if !row.license_assets_present {
                    return Err(format!(
                        "row {} is Complete but its declared license asset is absent",
                        row.name
                    ));
                }
            }
        }
        let complete = self
            .rows
            .iter()
            .filter(|row| row.result == RelocatedPackageDocsResultV1::Complete)
            .count();
        if self.aggregate.complete != complete
            || self.aggregate.incomplete != self.rows.len().saturating_sub(complete)
        {
            return Err("aggregate counts disagree with the rows".to_string());
        }
        let expected_aggregate = if complete == self.rows.len() {
            RelocatedPackageDocsResultV1::Complete
        } else {
            RelocatedPackageDocsResultV1::Incomplete
        };
        if self.aggregate.result != expected_aggregate {
            return Err("aggregate result disagrees with the rows".to_string());
        }
        Ok(())
    }
}
