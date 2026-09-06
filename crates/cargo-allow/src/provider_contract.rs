//! Discovery-only read-only provider descriptor for cargo-allow.
//!
//! The executable request, execution, and receipt protocol is intentionally
//! outside this module. The descriptor is the process-facing compatibility
//! surface consumed before those later operations are authorized.

use serde::Serialize;

const PROVIDER_CONTRACT_SCHEMA_ID: &str = "proof.cargo-allow-provider-contract.v1";
const PROVIDER_ID: &str = "proof.cargo-allow.v1";
const PRODUCT_NAME: &str = "cargo-allow";
const ACCESS_POSTURE: &str = "read_only";
const DISCOVERY_ORDER: [&str; 3] = [
    "explicit_environment",
    "compatibility_config",
    "path_lookup",
];
const FORBIDDEN_PATH_PREFIXES: [&str; 2] = ["target/", "crates/"];
const ENVIRONMENT_VARIABLE: &str = "CARGO_ALLOW_BIN";
const CONFIG_RELATIVE_PATH: &str = ".allow/compatibility/proof-delegation.toml";
const REQUIRED_CAPABILITIES: [&str; 2] = [
    "cargo-allow.check.no-new",
    "cargo-allow.capabilities.json",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderContractV1 {
    pub(crate) schema_id: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) provider_id: &'static str,
    pub(crate) product_name: &'static str,
    pub(crate) access_posture: &'static str,
    pub(crate) snapshot_bound: bool,
    pub(crate) discovery_order: [&'static str; 3],
    pub(crate) forbidden_path_prefixes: [&'static str; 2],
    pub(crate) environment_variable: &'static str,
    pub(crate) config_relative_path: &'static str,
    pub(crate) required_capabilities: [&'static str; 2],
}

pub(crate) const fn provider_contract() -> ProviderContractV1 {
    ProviderContractV1 {
        schema_id: PROVIDER_CONTRACT_SCHEMA_ID,
        schema_version: 1,
        provider_id: PROVIDER_ID,
        product_name: PRODUCT_NAME,
        access_posture: ACCESS_POSTURE,
        snapshot_bound: true,
        discovery_order: DISCOVERY_ORDER,
        forbidden_path_prefixes: FORBIDDEN_PATH_PREFIXES,
        environment_variable: ENVIRONMENT_VARIABLE,
        config_relative_path: CONFIG_RELATIVE_PATH,
        required_capabilities: REQUIRED_CAPABILITIES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_matches_discovery_v1() {
        let contract = provider_contract();
        assert_eq!(contract.schema_id, PROVIDER_CONTRACT_SCHEMA_ID);
        assert_eq!(contract.schema_version, 1);
        assert_eq!(contract.provider_id, PROVIDER_ID);
        assert_eq!(contract.product_name, PRODUCT_NAME);
        assert_eq!(contract.access_posture, ACCESS_POSTURE);
        assert!(contract.snapshot_bound);
        assert_eq!(contract.discovery_order, DISCOVERY_ORDER);
        assert_eq!(contract.forbidden_path_prefixes, FORBIDDEN_PATH_PREFIXES);
        assert_eq!(contract.environment_variable, ENVIRONMENT_VARIABLE);
        assert_eq!(contract.config_relative_path, CONFIG_RELATIVE_PATH);
        assert_eq!(contract.required_capabilities, REQUIRED_CAPABILITIES);
    }

    #[test]
    fn serialized_contract_exposes_only_discovery_metadata() -> Result<(), String> {
        let value = serde_json::to_value(provider_contract()).map_err(|error| error.to_string())?;
        let object = value
            .as_object()
            .ok_or_else(|| "provider descriptor must serialize as an object".to_string())?;
        if object.len() != 11 {
            return Err(format!(
                "provider descriptor field count changed: expected 11, got {}",
                object.len()
            ));
        }
        for executable_field in [
            "request_schema",
            "receipt_schema",
            "analysis_request",
            "analysis_receipt",
        ] {
            if object.contains_key(executable_field) {
                return Err(format!(
                    "discovery descriptor exposed executable field {executable_field}"
                ));
            }
        }
        Ok(())
    }
}
