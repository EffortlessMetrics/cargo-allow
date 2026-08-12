//! Generation-2 crate identity authority (#2921).
//!
//! Strict source-controlled identity authority where logical_id, Cargo package
//! name, and Rust library name are independently represented. This is a
//! Current V2 architecture authority. V1 remains available only through its
//! named historical reader.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

use crate::product_crates::config::CrateRole;

/// Schema version required for V2 manifests.
pub const ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION: &str = "2.0";

/// Authority generation required for V2 manifests.
pub const ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION: u32 = 2;

/// Generation-2 architecture manifest with independent identity fields (#2921).
///
/// Unlike V1 where logical_id, package name, and library name are conflated,
/// V2 represents each identity dimension independently. Every workspace crate
/// has exactly one [`CrateIdentityV2`] record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureManifestV2 {
    pub schema_version: String,
    pub authority_generation: u32,
    pub manifest_id: String,
    pub controlling_issue: u32,
    pub linked_move_ledger: String,
    pub crate_identity: Vec<CrateIdentityV2>,
}

/// Canonical per-crate identity record (#2921).
///
/// `logical_id` is the stable architecture and move-ledger identity.
/// `cargo_package_name` is the registry/package-selector identity.
/// `rust_library_name` is the import identity.
/// These may all differ (e.g., logical `repo-protocol` → package
/// `effortless-repo-protocol` → library `effortless_repo_protocol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateIdentityV2 {
    pub logical_id: String,
    pub workspace_path: String,
    pub workspace_dependency_aliases: Vec<String>,
    pub cargo_package_name: String,
    pub rust_library_name: String,
    pub product_or_shared_owner: String,
    pub crate_role: CrateRole,
}

// ---------------------------------------------------------------------------
// TOML deserialization shims (private, deny_unknown_fields)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchitectureManifestV2Toml {
    schema_version: Option<String>,
    authority_generation: Option<u32>,
    manifest_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_move_ledger: Option<String>,
    #[serde(default)]
    crate_identity: Vec<CrateIdentityV2Toml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrateIdentityV2Toml {
    logical_id: String,
    workspace_path: String,
    #[serde(default)]
    workspace_dependency_aliases: Vec<String>,
    cargo_package_name: String,
    rust_library_name: String,
    product_or_shared_owner: String,
    crate_role: String,
}

impl ArchitectureManifestV2Toml {
    fn into_manifest(self) -> CargoAllowResult<ArchitectureManifestV2> {
        let schema_version = self.schema_version.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 architecture manifest missing schema_version",
            )
        })?;
        if schema_version != ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported V2 architecture manifest schema_version `{schema_version}`; expected `{}`",
                    ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION
                ),
            ));
        }
        let authority_generation = self.authority_generation.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 architecture manifest missing authority_generation",
            )
        })?;
        if authority_generation != ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported V2 architecture manifest authority_generation `{authority_generation}`; expected `{}`",
                    ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION
                ),
            ));
        }
        let manifest_id = self.manifest_id.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 architecture manifest missing manifest_id",
            )
        })?;
        let controlling_issue = self.controlling_issue.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 architecture manifest missing controlling_issue",
            )
        })?;
        let linked_move_ledger = self.linked_move_ledger.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "V2 architecture manifest missing linked_move_ledger",
            )
        })?;

        let mut crate_identity = Vec::with_capacity(self.crate_identity.len());
        for entry in self.crate_identity {
            crate_identity.push(CrateIdentityV2 {
                logical_id: entry.logical_id,
                workspace_path: entry.workspace_path,
                workspace_dependency_aliases: entry.workspace_dependency_aliases,
                cargo_package_name: entry.cargo_package_name,
                rust_library_name: entry.rust_library_name,
                product_or_shared_owner: entry.product_or_shared_owner,
                crate_role: CrateRole::parse(&entry.crate_role)?,
            });
        }

        Ok(ArchitectureManifestV2 {
            schema_version,
            authority_generation,
            manifest_id,
            controlling_issue,
            linked_move_ledger,
            crate_identity,
        })
    }
}

// ---------------------------------------------------------------------------
// Public parse functions
// ---------------------------------------------------------------------------

/// Parse a V2 architecture manifest from TOML text (#2921).
///
/// Rejects `schema_version != "2.0"` and `authority_generation != 2` strictly.
/// V1 manifests (`schema_version = "1.0"`) are never accepted here — use
/// [`read_v1_as_historical`](crate::product_crates::read_v1_as_historical) for
/// migration. The link targets the public re-export; `v1_reader` itself is a
/// private module and cannot be linked from public documentation.
pub fn parse_architecture_manifest_v2(input: &str) -> CargoAllowResult<ArchitectureManifestV2> {
    parse_architecture_manifest_v2_at(None, input)
}

/// Parse a V2 architecture manifest with diagnostic span support.
pub fn parse_architecture_manifest_v2_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ArchitectureManifestV2> {
    let parsed = toml::from_str::<ArchitectureManifestV2Toml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse V2 architecture manifest TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_manifest()
}
