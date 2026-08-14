//! Fake provider for conformance harness (#2603-A).

use proof_protocol::{
    ProofCapabilityCatalogV1, ProofCapabilityKindV1, ProofCapabilityV1, ProofPlanV1,
};

use super::contracts::{ProofProviderV1, ProviderApiError};

pub const FAKE_PROOF_PROVIDER_ID: &str = "proof.fake-provider.v1";

pub struct FakeProofProviderV1 {
    catalog: ProofCapabilityCatalogV1,
}

impl FakeProofProviderV1 {
    pub fn new() -> Self {
        Self {
            catalog: ProofCapabilityCatalogV1::new(
                FAKE_PROOF_PROVIDER_ID,
                vec![ProofCapabilityV1 {
                    capability_id: "cargo-allow.check.no-new".to_string(),
                    kind: ProofCapabilityKindV1::CommandArgv,
                    program: "cargo-allow".to_string(),
                    statement: "Run cargo-allow no-new guard".to_string(),
                }],
            ),
        }
    }

    /// Create a fake provider with a specific provider ID (#3312).
    pub fn with_id(provider_id: &str) -> Self {
        Self {
            catalog: ProofCapabilityCatalogV1::new(
                provider_id,
                vec![ProofCapabilityV1 {
                    capability_id: "cargo-allow.check.no-new".to_string(),
                    kind: ProofCapabilityKindV1::CommandArgv,
                    program: "cargo-allow".to_string(),
                    statement: "Run cargo-allow no-new guard".to_string(),
                }],
            ),
        }
    }
}

impl Default for FakeProofProviderV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofProviderV1 for FakeProofProviderV1 {
    fn provider_id(&self) -> &str {
        &self.catalog.provider_id
    }

    fn capability_catalog(&self) -> &ProofCapabilityCatalogV1 {
        &self.catalog
    }

    fn validate_plan(&self, plan: &ProofPlanV1) -> Result<(), ProviderApiError> {
        if plan.commands.is_empty() {
            return Err(ProviderApiError::UnsupportedPlan {
                plan_id: plan.plan_id.clone(),
            });
        }
        Ok(())
    }
}
