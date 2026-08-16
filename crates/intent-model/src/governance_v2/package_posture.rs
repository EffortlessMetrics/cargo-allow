//! V2 package posture DTOs (#2942 step 1 / #3327).
//!
//! Pure authored package/version/publication/support posture facts and
//! candidate/CI membership. Mirrors the product-package topology authority
//! fields without any Cargo or filesystem access.

use serde::{Deserialize, Serialize};

/// Product support posture from the V2 topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProductPostureV2 {
    CargoAllowSupported,
    CargoIntentExperimental,
    CargoProofExperimental,
    SharedProtocolInternalOrStabilizing,
    SharedImplementationInternalOrExperimental,
    LegacyMigration,
}

impl ProductPostureV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllowSupported => "CargoAllowSupported",
            Self::CargoIntentExperimental => "CargoIntentExperimental",
            Self::CargoProofExperimental => "CargoProofExperimental",
            Self::SharedProtocolInternalOrStabilizing => "SharedProtocolInternalOrStabilizing",
            Self::SharedImplementationInternalOrExperimental => {
                "SharedImplementationInternalOrExperimental"
            }
            Self::LegacyMigration => "LegacyMigration",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "CargoAllowSupported" => Ok(Self::CargoAllowSupported),
            "CargoIntentExperimental" => Ok(Self::CargoIntentExperimental),
            "CargoProofExperimental" => Ok(Self::CargoProofExperimental),
            "SharedProtocolInternalOrStabilizing" => Ok(Self::SharedProtocolInternalOrStabilizing),
            "SharedImplementationInternalOrExperimental" => {
                Ok(Self::SharedImplementationInternalOrExperimental)
            }
            "LegacyMigration" => Ok(Self::LegacyMigration),
            other => Err(format!("unsupported product posture `{other}`")),
        }
    }
}

/// Where a package's version number comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionSourceV2 {
    Explicit,
    WorkspaceProduct,
}

impl VersionSourceV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "Explicit",
            Self::WorkspaceProduct => "WorkspaceProduct",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "Explicit" => Ok(Self::Explicit),
            "WorkspaceProduct" => Ok(Self::WorkspaceProduct),
            other => Err(format!("unsupported version source `{other}`")),
        }
    }
}

/// Publication state of a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PublicationStateV2 {
    UnpublishedInternal,
    QualifiedForPublication,
    Published,
    Retired,
}

impl PublicationStateV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnpublishedInternal => "UnpublishedInternal",
            Self::QualifiedForPublication => "QualifiedForPublication",
            Self::Published => "Published",
            Self::Retired => "Retired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "UnpublishedInternal" => Ok(Self::UnpublishedInternal),
            "QualifiedForPublication" => Ok(Self::QualifiedForPublication),
            "Published" => Ok(Self::Published),
            "Retired" => Ok(Self::Retired),
            other => Err(format!("unsupported publication state `{other}`")),
        }
    }
}

/// Candidate/CI membership for a package (#2604 authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateMembershipV2 {
    /// Included in the exact release candidate package set.
    pub candidate_inclusion: bool,
    /// Publishable to the registry when authorized.
    pub publish: bool,
}

/// Authored package posture for one logical package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernancePackagePostureV2 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub version_line: String,
    pub product_family: String,
    pub posture: ProductPostureV2,
    pub package_version: String,
    pub version_source: VersionSourceV2,
    pub publication_state: PublicationStateV2,
    pub membership: CandidateMembershipV2,
    pub release_order: u32,
    /// Lane in `docs/ci-lanes.toml` that proves this package (#3365).
    pub ci_lane: String,
    /// Package support tier mirroring the product support matrix (#3365).
    pub support_tier: String,
    /// Workspace directories owned by the package for release-asset
    /// qualification; empty means no release assets (#3365).
    pub asset_roots: Vec<String>,
    /// Repository the package lands in under the #2559 repository split
    /// (#3365).
    pub extraction_destination: String,
}

/// Recognized repository-extraction destinations (#2559/#3365). Shared
/// protocol crates remain hosted by the cargo-allow repository.
pub const EXTRACTION_DESTINATIONS: &[&str] = &["cargo-allow", "cargo-intent", "cargo-proof"];

/// Recognized package support tiers (#3365), mirroring the product
/// support matrix vocabulary.
pub const SUPPORT_TIERS: &[&str] = &[
    "supported",
    "experimental-opt-in",
    "internal-stabilizing",
    "legacy",
];

impl GovernancePackagePostureV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.logical_id.trim().is_empty() {
            return Err("package posture requires a non-empty logical_id".into());
        }
        if self.cargo_package_name.trim().is_empty() {
            return Err(format!(
                "package `{}` requires a non-empty cargo_package_name",
                self.logical_id
            ));
        }
        if self.version_line.trim().is_empty() {
            return Err(format!(
                "package `{}` requires a non-empty version_line",
                self.logical_id
            ));
        }
        if self.package_version.trim().is_empty() {
            return Err(format!(
                "package `{}` requires a non-empty package_version",
                self.logical_id
            ));
        }
        if self.ci_lane.trim().is_empty() {
            return Err(format!(
                "package `{}` requires a non-empty ci_lane (#3365)",
                self.logical_id
            ));
        }
        if !SUPPORT_TIERS.contains(&self.support_tier.as_str()) {
            return Err(format!(
                "package `{}` has unrecognized support_tier `{}`; expected one of {SUPPORT_TIERS:?}",
                self.logical_id, self.support_tier
            ));
        }
        if !EXTRACTION_DESTINATIONS.contains(&self.extraction_destination.as_str()) {
            return Err(format!(
                "package `{}` has unrecognized extraction_destination `{}`; expected one of {EXTRACTION_DESTINATIONS:?}",
                self.logical_id, self.extraction_destination
            ));
        }
        for root in &self.asset_roots {
            if root.trim().is_empty()
                || root.starts_with('/')
                || root.starts_with("..")
                || root.contains(':')
            {
                return Err(format!(
                    "package `{}` asset_root `{root}` must be a non-empty workspace-relative directory path",
                    self.logical_id
                ));
            }
        }
        Ok(())
    }
}
