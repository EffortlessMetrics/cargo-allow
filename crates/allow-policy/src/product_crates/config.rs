use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateRole {
    CargoAllowCore,
    CargoIntent,
    CargoProof,
    SharedProtocol,
    SharedSnapshot,
    ProviderAdapter,
    CompatibilityAdapter,
    LegacyMigration,
    TestFixtureOnly,
}

impl CrateRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllowCore => "CargoAllowCore",
            Self::CargoIntent => "CargoIntent",
            Self::CargoProof => "CargoProof",
            Self::SharedProtocol => "SharedProtocol",
            Self::SharedSnapshot => "SharedSnapshot",
            Self::ProviderAdapter => "ProviderAdapter",
            Self::CompatibilityAdapter => "CompatibilityAdapter",
            Self::LegacyMigration => "LegacyMigration",
            Self::TestFixtureOnly => "TestFixtureOnly",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "CargoAllowCore" => Ok(Self::CargoAllowCore),
            "CargoIntent" => Ok(Self::CargoIntent),
            "CargoProof" => Ok(Self::CargoProof),
            "SharedProtocol" => Ok(Self::SharedProtocol),
            "SharedSnapshot" => Ok(Self::SharedSnapshot),
            "ProviderAdapter" => Ok(Self::ProviderAdapter),
            "CompatibilityAdapter" => Ok(Self::CompatibilityAdapter),
            "LegacyMigration" => Ok(Self::LegacyMigration),
            "TestFixtureOnly" => Ok(Self::TestFixtureOnly),
            other => Err(CargoAllowError::new(format!(
                "unsupported crate role `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDefinition {
    pub id: String,
    pub binary: Option<String>,
    pub owned_crates: Vec<String>,
    pub forbid_product_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCrateDefinition {
    pub name: String,
    pub role: CrateRole,
    pub allowed_domain_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCrate {
    pub name: String,
    pub owner_product: String,
    pub role: CrateRole,
    pub stage_issue: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenCrateDependency {
    pub from: String,
    pub to: String,
    pub repair_hint: Option<String>,
}

/// A converged dependency path that must stay declared (#2936 / #3317).
///
/// Records the final obligation-input authority: proof-engine must depend on
/// intent-protocol, and the deleted proof-owned obligation model must not
/// silently sever that path. `from_package` resolves the logical crate name
/// to its cargo package name when they differ (e.g. proof-engine is
/// published as proof-orchestrator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredCrateDependency {
    pub from: String,
    pub from_package: Option<String>,
    pub to: String,
    pub rationale_issue: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub controlling_issue: u32,
    pub linked_move_ledger: String,
    pub product: Vec<ProductDefinition>,
    pub shared_crate: Vec<SharedCrateDefinition>,
    pub planned_crate: Vec<PlannedCrate>,
    pub forbidden_crate_dependency: Vec<ForbiddenCrateDependency>,
    pub required_crate_dependency: Vec<RequiredCrateDependency>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchitectureManifestToml {
    schema_version: Option<String>,
    manifest_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_move_ledger: Option<String>,
    #[serde(default)]
    product: Vec<ProductDefinitionToml>,
    #[serde(default)]
    shared_crate: Vec<SharedCrateDefinitionToml>,
    #[serde(default)]
    planned_crate: Vec<PlannedCrateToml>,
    #[serde(default)]
    forbidden_crate_dependency: Vec<ForbiddenCrateDependencyToml>,
    #[serde(default)]
    required_crate_dependency: Vec<RequiredCrateDependencyToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductDefinitionToml {
    id: String,
    binary: Option<String>,
    #[serde(default)]
    owned_crates: Vec<String>,
    #[serde(default)]
    forbid_product_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedCrateDefinitionToml {
    name: String,
    role: String,
    #[serde(default)]
    allowed_domain_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlannedCrateToml {
    name: String,
    owner_product: String,
    role: String,
    stage_issue: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForbiddenCrateDependencyToml {
    from: String,
    to: String,
    repair_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiredCrateDependencyToml {
    from: String,
    from_package: Option<String>,
    to: String,
    rationale_issue: Option<u32>,
}

impl ArchitectureManifestToml {
    fn into_manifest(self) -> CargoAllowResult<ArchitectureManifest> {
        let schema_version = self.schema_version.unwrap_or_else(|| "1.0".to_string());
        let manifest_id = self
            .manifest_id
            .ok_or_else(|| CargoAllowError::new("architecture manifest missing manifest_id"))?;
        let controlling_issue = self.controlling_issue.ok_or_else(|| {
            CargoAllowError::new("architecture manifest missing controlling_issue")
        })?;
        let linked_move_ledger = self.linked_move_ledger.ok_or_else(|| {
            CargoAllowError::new("architecture manifest missing linked_move_ledger")
        })?;

        let product = self
            .product
            .into_iter()
            .map(|entry| ProductDefinition {
                id: entry.id,
                binary: entry.binary,
                owned_crates: entry.owned_crates,
                forbid_product_dependencies: entry.forbid_product_dependencies,
            })
            .collect();

        let mut shared_crate = Vec::with_capacity(self.shared_crate.len());
        for entry in self.shared_crate {
            shared_crate.push(SharedCrateDefinition {
                name: entry.name,
                role: CrateRole::parse(&entry.role)?,
                allowed_domain_dependencies: entry.allowed_domain_dependencies,
            });
        }

        let mut planned_crate = Vec::with_capacity(self.planned_crate.len());
        for entry in self.planned_crate {
            planned_crate.push(PlannedCrate {
                name: entry.name,
                owner_product: entry.owner_product,
                role: CrateRole::parse(&entry.role)?,
                stage_issue: entry.stage_issue,
            });
        }

        let forbidden_crate_dependency = self
            .forbidden_crate_dependency
            .into_iter()
            .map(|entry| ForbiddenCrateDependency {
                from: entry.from,
                to: entry.to,
                repair_hint: entry.repair_hint,
            })
            .collect();

        let required_crate_dependency = self
            .required_crate_dependency
            .into_iter()
            .map(|entry| RequiredCrateDependency {
                from: entry.from,
                from_package: entry.from_package,
                to: entry.to,
                rationale_issue: entry.rationale_issue,
            })
            .collect();

        Ok(ArchitectureManifest {
            schema_version,
            manifest_id,
            controlling_issue,
            linked_move_ledger,
            product,
            shared_crate,
            planned_crate,
            forbidden_crate_dependency,
            required_crate_dependency,
        })
    }
}

pub fn parse_architecture_manifest(input: &str) -> CargoAllowResult<ArchitectureManifest> {
    parse_architecture_manifest_at(None, input)
}

pub fn parse_architecture_manifest_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ArchitectureManifest> {
    let parsed = toml::from_str::<ArchitectureManifestToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse architecture manifest TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_manifest()
}
