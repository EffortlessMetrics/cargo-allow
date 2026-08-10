//! Generation-2 package topology with explicit identity and version fields
//! (#2921).
//!
//! Extends V1 topology with per-package version, publication state, and
//! logical_id linkage to the V2 architecture authority. Report-only — does
//! not replace V1 enforcement.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

use crate::product_packages::config::PackagePosture;

/// Schema version required for V2 topology.
pub const PRODUCT_PACKAGE_TOPOLOGY_V2_SCHEMA_VERSION: &str = "2.0";

/// Authority generation required for V2 topology.
pub const PRODUCT_PACKAGE_TOPOLOGY_V2_AUTHORITY_GENERATION: u32 = 2;

/// Generation-2 package topology with explicit version and identity fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductPackageTopologyV2 {
    pub schema_version: String,
    pub authority_generation: u32,
    pub topology_id: String,
    pub controlling_issue: u32,
    pub linked_architecture_manifest: String,
    pub package: Vec<PackageTopologyEntryV2>,
}

/// V2 package topology entry with per-package version, publication state, and
/// logical_id linkage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTopologyEntryV2 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub product_family: String,
    pub posture: PackagePosture,
    pub package_version: String,
    pub version_source: VersionSourceV2,
    /// Independent version line for non-lockstep compatibility (#3362).
    /// Format: "<product-family>-<major>.<minor>" (e.g., "cargo-allow-0.2").
    pub version_line: String,
    pub publication_state: PublicationStateV2,
    pub publish: bool,
    pub candidate_inclusion: bool,
    pub release_order: u32,
}

/// Where a package's version is sourced from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSourceV2 {
    WorkspaceProduct,
    Explicit,
}

impl VersionSourceV2 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceProduct => "WorkspaceProduct",
            Self::Explicit => "Explicit",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "WorkspaceProduct" => Ok(Self::WorkspaceProduct),
            "Explicit" => Ok(Self::Explicit),
            other => Err(CargoAllowError::new(format!(
                "unsupported version_source `{other}`"
            ))),
        }
    }
}

/// Publication lifecycle state for a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStateV2 {
    UnpublishedInternal,
    Published,
    Retired,
}

impl PublicationStateV2 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnpublishedInternal => "UnpublishedInternal",
            Self::Published => "Published",
            Self::Retired => "Retired",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "UnpublishedInternal" => Ok(Self::UnpublishedInternal),
            "Published" => Ok(Self::Published),
            "Retired" => Ok(Self::Retired),
            other => Err(CargoAllowError::new(format!(
                "unsupported publication_state `{other}`"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// TOML deserialization shims (private, deny_unknown_fields)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductPackageTopologyV2Toml {
    schema_version: Option<String>,
    authority_generation: Option<u32>,
    topology_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_architecture_manifest: Option<String>,
    #[serde(default)]
    package: Vec<PackageTopologyEntryV2Toml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageTopologyEntryV2Toml {
    logical_id: String,
    cargo_package_name: String,
    product_family: String,
    posture: String,
    package_version: String,
    version_source: String,
    version_line: String,
    publication_state: String,
    publish: bool,
    candidate_inclusion: bool,
    release_order: u32,
}

impl ProductPackageTopologyV2Toml {
    fn into_topology(self) -> CargoAllowResult<ProductPackageTopologyV2> {
        let schema_version = self.schema_version.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 package topology missing schema_version",
            )
        })?;
        if schema_version != PRODUCT_PACKAGE_TOPOLOGY_V2_SCHEMA_VERSION {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported V2 package topology schema_version `{schema_version}`; expected `{}`",
                    PRODUCT_PACKAGE_TOPOLOGY_V2_SCHEMA_VERSION
                ),
            ));
        }
        let authority_generation = self.authority_generation.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 package topology missing authority_generation",
            )
        })?;
        if authority_generation != PRODUCT_PACKAGE_TOPOLOGY_V2_AUTHORITY_GENERATION {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported V2 package topology authority_generation `{authority_generation}`; expected `{}`",
                    PRODUCT_PACKAGE_TOPOLOGY_V2_AUTHORITY_GENERATION
                ),
            ));
        }
        let topology_id = self.topology_id.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 package topology missing topology_id",
            )
        })?;
        let controlling_issue = self.controlling_issue.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 package topology missing controlling_issue",
            )
        })?;
        let linked_architecture_manifest = self.linked_architecture_manifest.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 package topology missing linked_architecture_manifest",
            )
        })?;

        let mut package = Vec::with_capacity(self.package.len());
        for entry in self.package {
            package.push(PackageTopologyEntryV2 {
                logical_id: entry.logical_id,
                cargo_package_name: entry.cargo_package_name,
                product_family: entry.product_family,
                posture: PackagePosture::parse(&entry.posture)?,
                package_version: entry.package_version,
                version_source: VersionSourceV2::parse(&entry.version_source)?,
                version_line: entry.version_line,
                publication_state: PublicationStateV2::parse(&entry.publication_state)?,
                publish: entry.publish,
                candidate_inclusion: entry.candidate_inclusion,
                release_order: entry.release_order,
            });
        }

        Ok(ProductPackageTopologyV2 {
            schema_version,
            authority_generation,
            topology_id,
            controlling_issue,
            linked_architecture_manifest,
            package,
        })
    }
}

// ---------------------------------------------------------------------------
// Public parse functions
// ---------------------------------------------------------------------------

pub fn parse_product_package_topology_v2(
    input: &str,
) -> CargoAllowResult<ProductPackageTopologyV2> {
    parse_product_package_topology_v2_at(None, input)
}

pub fn parse_product_package_topology_v2_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ProductPackageTopologyV2> {
    let parsed = toml::from_str::<ProductPackageTopologyV2Toml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse V2 package topology TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_topology()
}
