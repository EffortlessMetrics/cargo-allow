//! Provider-neutral proof execution API (#2603-A).

use proof_protocol::{
    ProofCapabilityCatalogV1, ProofPlanError, ProofPlanV1, validate_capability_catalog,
    validate_proof_plan,
};

pub const PROOF_PROVIDER_API_SCHEMA_ID: &str = "proof.provider-api.v1";

pub trait ProofProviderV1 {
    fn provider_id(&self) -> &str;
    fn capability_catalog(&self) -> &ProofCapabilityCatalogV1;
    fn validate_plan(&self, plan: &ProofPlanV1) -> Result<(), ProviderApiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderApiError {
    ProofPlan(ProofPlanError),
    CapabilityCatalogInvalid,
    UnsupportedPlan { plan_id: String },
}

impl ProviderApiError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ProofPlan(_) => "proof_plan_invalid",
            Self::CapabilityCatalogInvalid => "capability_catalog_invalid",
            Self::UnsupportedPlan { .. } => "unsupported_plan",
        }
    }
}

pub fn validate_provider_surface(provider: &dyn ProofProviderV1) -> Result<(), ProviderApiError> {
    validate_capability_catalog(provider.capability_catalog())
        .map_err(|_| ProviderApiError::CapabilityCatalogInvalid)?;
    Ok(())
}

pub fn validate_provider_plan(
    provider: &dyn ProofProviderV1,
    plan: &ProofPlanV1,
) -> Result<(), ProviderApiError> {
    validate_provider_surface(provider)?;
    validate_proof_plan(plan).map_err(ProviderApiError::ProofPlan)?;
    provider.validate_plan(plan)
}
