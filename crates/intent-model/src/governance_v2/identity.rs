//! V2 governance identity DTOs (#2942 step 1 / #3327).
//!
//! Pure authored identity facts for the repository's governance authority:
//! crate identity, owner/role, component kind, and target disposition. No
//! Cargo process handles, filesystem/Git access, ambient workspace state, or
//! cargo-allow ledger semantics live here.

use serde::{Deserialize, Serialize};

/// Product or shared ownership of a crate (#2580 authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernanceOwnerV2 {
    CargoAllow,
    CargoIntent,
    CargoProof,
    Shared,
}

impl GovernanceOwnerV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllow => "cargo-allow",
            Self::CargoIntent => "cargo-intent",
            Self::CargoProof => "cargo-proof",
            Self::Shared => "shared",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "cargo-allow" => Ok(Self::CargoAllow),
            "cargo-intent" => Ok(Self::CargoIntent),
            "cargo-proof" => Ok(Self::CargoProof),
            "shared" => Ok(Self::Shared),
            other => Err(format!("unsupported governance owner `{other}`")),
        }
    }
}

/// Role a crate plays in its product family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernanceCrateRoleV2 {
    CargoAllowCore,
    CargoIntent,
    CargoProof,
    SharedProtocol,
    SharedSnapshot,
    LegacyMigration,
}

impl GovernanceCrateRoleV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllowCore => "CargoAllowCore",
            Self::CargoIntent => "CargoIntent",
            Self::CargoProof => "CargoProof",
            Self::SharedProtocol => "SharedProtocol",
            Self::SharedSnapshot => "SharedSnapshot",
            Self::LegacyMigration => "LegacyMigration",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "CargoAllowCore" => Ok(Self::CargoAllowCore),
            "CargoIntent" => Ok(Self::CargoIntent),
            "CargoProof" => Ok(Self::CargoProof),
            "SharedProtocol" => Ok(Self::SharedProtocol),
            "SharedSnapshot" => Ok(Self::SharedSnapshot),
            "LegacyMigration" => Ok(Self::LegacyMigration),
            other => Err(format!("unsupported governance crate role `{other}`")),
        }
    }
}

/// What kind of governance component a record describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernanceComponentKindV2 {
    Crate,
    Package,
    Module,
    Fixture,
    Workflow,
}

impl GovernanceComponentKindV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crate => "crate",
            Self::Package => "package",
            Self::Module => "module",
            Self::Fixture => "fixture",
            Self::Workflow => "workflow",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "crate" => Ok(Self::Crate),
            "package" => Ok(Self::Package),
            "module" => Ok(Self::Module),
            "fixture" => Ok(Self::Fixture),
            "workflow" => Ok(Self::Workflow),
            other => Err(format!("unsupported governance component kind `{other}`")),
        }
    }
}

/// Authored target disposition for a component in the convergence plan
/// (#2934 authority convergence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetDispositionV2 {
    RetainPackage,
    CollapseIntoPackage,
    CompatibilityOnly,
    DeferUntilEvidence,
    RemoveAfterCutover,
}

impl TargetDispositionV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetainPackage => "retain_package",
            Self::CollapseIntoPackage => "collapse_into_package",
            Self::CompatibilityOnly => "compatibility_only",
            Self::DeferUntilEvidence => "defer_until_evidence",
            Self::RemoveAfterCutover => "remove_after_cutover",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "retain_package" => Ok(Self::RetainPackage),
            "collapse_into_package" => Ok(Self::CollapseIntoPackage),
            "compatibility_only" => Ok(Self::CompatibilityOnly),
            "defer_until_evidence" => Ok(Self::DeferUntilEvidence),
            "remove_after_cutover" => Ok(Self::RemoveAfterCutover),
            other => Err(format!("unsupported target disposition `{other}`")),
        }
    }
}

/// Canonical logical/current/target crate identity (#2921).
///
/// `logical_id` is the stable governance name (e.g. `proof-engine`);
/// `cargo_package_name` is the published package name when they differ
/// (e.g. `proof-orchestrator`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceCrateIdentityV2 {
    pub logical_id: String,
    pub workspace_path: String,
    #[serde(default)]
    pub workspace_dependency_aliases: Vec<String>,
    pub cargo_package_name: String,
    pub rust_library_name: String,
    pub owner: GovernanceOwnerV2,
    pub role: GovernanceCrateRoleV2,
}

impl GovernanceCrateIdentityV2 {
    /// Strict structural validation: required identity fields must be
    /// non-empty and distinct where the authority requires it.
    pub fn validate(&self) -> Result<(), String> {
        if self.logical_id.trim().is_empty() {
            return Err("crate identity requires a non-empty logical_id".into());
        }
        if self.workspace_path.trim().is_empty() {
            return Err(format!(
                "crate `{}` requires a non-empty workspace_path",
                self.logical_id
            ));
        }
        if self.cargo_package_name.trim().is_empty() {
            return Err(format!(
                "crate `{}` requires a non-empty cargo_package_name",
                self.logical_id
            ));
        }
        if self.rust_library_name.trim().is_empty() {
            return Err(format!(
                "crate `{}` requires a non-empty rust_library_name",
                self.logical_id
            ));
        }
        Ok(())
    }
}
