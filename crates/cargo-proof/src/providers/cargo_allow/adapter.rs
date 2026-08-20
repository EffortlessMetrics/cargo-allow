//! cargo-allow ProofProviderV1 implementation (#2554).

use proof_engine::{ProofProviderV1, ProviderApiError};
use proof_protocol::{
    ProofCapabilityCatalogV1, ProofCapabilityKindV1, ProofCapabilityV1, ProofPlanV1,
};

use super::contract::{
    CARGO_ALLOW_PROOF_PROVIDER_ID, default_cargo_allow_provider_contract,
    validate_provider_contract,
};
use super::discovery::{
    CargoAllowProviderRequest, CargoAllowProviderResolution, discover_cargo_allow_provider,
};
use super::process_protocol::{ProcessProtocolError, validate_process_protocol_plan};

pub struct CargoAllowProofProviderV1 {
    catalog: ProofCapabilityCatalogV1,
    contract_validated: bool,
    resolution: Option<CargoAllowProviderResolution>,
}

impl CargoAllowProofProviderV1 {
    pub fn new() -> Self {
        let contract = default_cargo_allow_provider_contract();
        let _ = validate_provider_contract(&contract);
        Self {
            catalog: capability_catalog_from_contract(&contract),
            contract_validated: validate_provider_contract(&contract).is_ok(),
            resolution: None,
        }
    }

    pub fn with_discovery(
        request: &CargoAllowProviderRequest<'_>,
    ) -> Result<Self, super::discovery::CargoAllowProviderFailure> {
        let contract = default_cargo_allow_provider_contract();
        validate_provider_contract(&contract).map_err(|_| {
            super::discovery::CargoAllowProviderFailure::new(
                super::discovery::CargoAllowProviderFailureClass::MalformedConfig,
                "provider contract invalid",
            )
        })?;
        let resolution = discover_cargo_allow_provider(request)?;
        Ok(Self {
            catalog: capability_catalog_from_contract(&contract),
            contract_validated: true,
            resolution: Some(resolution),
        })
    }

    pub fn resolution(&self) -> Option<&CargoAllowProviderResolution> {
        self.resolution.as_ref()
    }
}

impl Default for CargoAllowProofProviderV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofProviderV1 for CargoAllowProofProviderV1 {
    fn provider_id(&self) -> &str {
        CARGO_ALLOW_PROOF_PROVIDER_ID
    }

    fn capability_catalog(&self) -> &ProofCapabilityCatalogV1 {
        &self.catalog
    }

    fn validate_plan(&self, plan: &ProofPlanV1) -> Result<(), ProviderApiError> {
        if !self.contract_validated {
            return Err(ProviderApiError::CapabilityCatalogInvalid);
        }
        validate_process_protocol_plan(plan).map_err(|err| match err {
            ProcessProtocolError::EmptyPlan | ProcessProtocolError::UnsupportedPlan { .. } => {
                ProviderApiError::UnsupportedPlan {
                    plan_id: plan.plan_id.clone(),
                }
            }
            ProcessProtocolError::RegistryInvalid => ProviderApiError::CapabilityCatalogInvalid,
            ProcessProtocolError::UnsupportedCommand { .. } => ProviderApiError::UnsupportedPlan {
                plan_id: plan.plan_id.clone(),
            },
        })
    }
}

fn capability_catalog_from_contract(
    contract: &super::contract::CargoAllowProviderContractV1,
) -> ProofCapabilityCatalogV1 {
    ProofCapabilityCatalogV1::new(
        CARGO_ALLOW_PROOF_PROVIDER_ID,
        contract
            .required_capabilities
            .iter()
            .map(|capability_id| {
                let is_capability_report = capability_id == "cargo-allow.capabilities.json";
                ProofCapabilityV1 {
                    capability_id: capability_id.clone(),
                    kind: if is_capability_report {
                        ProofCapabilityKindV1::StaticReport
                    } else {
                        ProofCapabilityKindV1::CommandArgv
                    },
                    program: contract.product_name.clone(),
                    statement: if is_capability_report {
                        "Read the cargo-allow.sensor-capabilities.v1 report via the public process protocol".to_string()
                    } else {
                        format!("Run {capability_id} via public process protocol")
                    },
                }
            })
            .collect(),
    )
}
