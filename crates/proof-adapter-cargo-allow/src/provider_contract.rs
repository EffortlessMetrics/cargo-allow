//! Snapshot-bound read-only cargo-allow provider contract (#2567).

use serde::Deserialize;

pub const CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID: &str = "proof.cargo-allow-provider-contract.v1";

pub const CARGO_ALLOW_PROOF_PROVIDER_ID: &str = "proof.cargo-allow.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderContractError {
    EmptySchemaId,
    UnsupportedSchemaId { schema_id: String },
    ReadWriteConflict,
    MissingCapability,
    UnknownCapability { capability_id: String },
}

impl ProviderContractError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptySchemaId => "empty_schema_id",
            Self::UnsupportedSchemaId { .. } => "unsupported_schema_id",
            Self::ReadWriteConflict => "read_write_conflict",
            Self::MissingCapability => "missing_capability",
            Self::UnknownCapability { .. } => "unknown_capability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAccessPostureV1 {
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CargoAllowProviderContractV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub provider_id: String,
    pub product_name: String,
    pub access_posture: ProviderAccessPostureV1,
    pub snapshot_bound: bool,
    pub discovery_order: Vec<String>,
    pub forbidden_path_prefixes: Vec<String>,
    pub environment_variable: String,
    pub config_relative_path: String,
    pub required_capabilities: Vec<String>,
}

pub fn default_cargo_allow_provider_contract() -> CargoAllowProviderContractV1 {
    CargoAllowProviderContractV1 {
        schema_id: CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID.to_string(),
        schema_version: 1,
        provider_id: CARGO_ALLOW_PROOF_PROVIDER_ID.to_string(),
        product_name: "cargo-allow".to_string(),
        access_posture: ProviderAccessPostureV1::ReadOnly,
        snapshot_bound: true,
        discovery_order: vec![
            "explicit_environment".to_string(),
            "compatibility_config".to_string(),
            "path_lookup".to_string(),
        ],
        forbidden_path_prefixes: vec!["target/".to_string(), "crates/".to_string()],
        environment_variable: "CARGO_ALLOW_BIN".to_string(),
        config_relative_path: ".allow/compatibility/proof-delegation.toml".to_string(),
        required_capabilities: vec![
            "cargo-allow.check.no-new".to_string(),
            "cargo-allow.capabilities.json".to_string(),
        ],
    }
}

pub fn validate_provider_contract(
    contract: &CargoAllowProviderContractV1,
) -> Result<(), ProviderContractError> {
    if contract.schema_id.trim().is_empty() {
        return Err(ProviderContractError::EmptySchemaId);
    }
    if contract.schema_id != CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID {
        return Err(ProviderContractError::UnsupportedSchemaId {
            schema_id: contract.schema_id.clone(),
        });
    }
    if contract.access_posture != ProviderAccessPostureV1::ReadOnly {
        return Err(ProviderContractError::ReadWriteConflict);
    }
    if !contract.snapshot_bound {
        return Err(ProviderContractError::ReadWriteConflict);
    }
    if contract.required_capabilities.is_empty() {
        return Err(ProviderContractError::MissingCapability);
    }
    if !contract
        .required_capabilities
        .iter()
        .any(|cap| cap == "cargo-allow.check.no-new")
    {
        return Err(ProviderContractError::MissingCapability);
    }
    Ok(())
}
