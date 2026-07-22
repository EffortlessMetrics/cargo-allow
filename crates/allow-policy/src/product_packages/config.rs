use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePosture {
    CargoAllowSupported,
    CargoIntentExperimental,
    CargoProofExperimental,
    SharedProtocolInternalOrStabilizing,
    SharedImplementationInternalOrExperimental,
    ProviderAdapterExperimental,
    CompatibilityOnly,
    LegacyMigration,
    FixtureOrTestOnly,
    UnpublishedInternal,
}

impl PackagePosture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllowSupported => "CargoAllowSupported",
            Self::CargoIntentExperimental => "CargoIntentExperimental",
            Self::CargoProofExperimental => "CargoProofExperimental",
            Self::SharedProtocolInternalOrStabilizing => "SharedProtocolInternalOrStabilizing",
            Self::SharedImplementationInternalOrExperimental => {
                "SharedImplementationInternalOrExperimental"
            }
            Self::ProviderAdapterExperimental => "ProviderAdapterExperimental",
            Self::CompatibilityOnly => "CompatibilityOnly",
            Self::LegacyMigration => "LegacyMigration",
            Self::FixtureOrTestOnly => "FixtureOrTestOnly",
            Self::UnpublishedInternal => "UnpublishedInternal",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "CargoAllowSupported" => Ok(Self::CargoAllowSupported),
            "CargoIntentExperimental" => Ok(Self::CargoIntentExperimental),
            "CargoProofExperimental" => Ok(Self::CargoProofExperimental),
            "SharedProtocolInternalOrStabilizing" => Ok(Self::SharedProtocolInternalOrStabilizing),
            "SharedImplementationInternalOrExperimental" => {
                Ok(Self::SharedImplementationInternalOrExperimental)
            }
            "ProviderAdapterExperimental" => Ok(Self::ProviderAdapterExperimental),
            "CompatibilityOnly" => Ok(Self::CompatibilityOnly),
            "LegacyMigration" => Ok(Self::LegacyMigration),
            "FixtureOrTestOnly" => Ok(Self::FixtureOrTestOnly),
            "UnpublishedInternal" => Ok(Self::UnpublishedInternal),
            other => Err(CargoAllowError::new(format!(
                "unsupported package posture `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTopologyEntry {
    pub package: String,
    pub product_family: String,
    pub posture: PackagePosture,
    pub publish: bool,
    pub candidate_inclusion: bool,
    pub release_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductPackageTopology {
    pub schema_version: String,
    pub topology_id: String,
    pub controlling_issue: u32,
    pub linked_architecture_manifest: String,
    pub package: Vec<PackageTopologyEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductPackageTopologyToml {
    schema_version: Option<String>,
    topology_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_architecture_manifest: Option<String>,
    #[serde(default)]
    package: Vec<PackageTopologyEntryToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageTopologyEntryToml {
    package: String,
    product_family: String,
    posture: String,
    publish: bool,
    candidate_inclusion: bool,
    release_order: u32,
}

impl ProductPackageTopologyToml {
    fn into_topology(self) -> CargoAllowResult<ProductPackageTopology> {
        let topology_id = self
            .topology_id
            .ok_or_else(|| CargoAllowError::new("package topology missing topology_id"))?;
        let controlling_issue = self
            .controlling_issue
            .ok_or_else(|| CargoAllowError::new("package topology missing controlling_issue"))?;
        let linked_architecture_manifest = self.linked_architecture_manifest.ok_or_else(|| {
            CargoAllowError::new("package topology missing linked_architecture_manifest")
        })?;

        let mut package = Vec::with_capacity(self.package.len());
        for entry in self.package {
            package.push(PackageTopologyEntry {
                package: entry.package,
                product_family: entry.product_family,
                posture: PackagePosture::parse(&entry.posture)?,
                publish: entry.publish,
                candidate_inclusion: entry.candidate_inclusion,
                release_order: entry.release_order,
            });
        }

        Ok(ProductPackageTopology {
            schema_version: self.schema_version.unwrap_or_else(|| "1.0".to_string()),
            topology_id,
            controlling_issue,
            linked_architecture_manifest,
            package,
        })
    }
}

pub fn parse_product_package_topology(input: &str) -> CargoAllowResult<ProductPackageTopology> {
    parse_product_package_topology_at(None, input)
}

pub fn parse_product_package_topology_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ProductPackageTopology> {
    let parsed = toml::from_str::<ProductPackageTopologyToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse package topology TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_topology()
}
