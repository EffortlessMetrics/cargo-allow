//! RIPR ProofProviderV1 implementation (#2556).

use proof_engine::{ProofProviderV1, ProviderApiError};
use proof_protocol::{
    ProofCapabilityCatalogV1, ProofCapabilityKindV1, ProofCapabilityV1, ProofPlanV1,
};

pub const RIPR_PROOF_PROVIDER_ID: &str = "proof.ripr.v1";

pub struct RiprProofProviderV1 {
    catalog: ProofCapabilityCatalogV1,
}

impl RiprProofProviderV1 {
    pub fn new() -> Self {
        Self {
            catalog: ProofCapabilityCatalogV1::new(
                RIPR_PROOF_PROVIDER_ID,
                vec![
                    ProofCapabilityV1 {
                        capability_id: "ripr.grip-receipt.validate".to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "ripr".to_string(),
                        statement: "Validate captured RIPR TestGripSummary receipt".to_string(),
                    },
                    ProofCapabilityV1 {
                        capability_id: "ripr.requirement-grip.compare".to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "ripr".to_string(),
                        statement: "Compare intent evidence purpose with validated RIPR facts"
                            .to_string(),
                    },
                ],
            ),
        }
    }
}

impl Default for RiprProofProviderV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofProviderV1 for RiprProofProviderV1 {
    fn provider_id(&self) -> &str {
        RIPR_PROOF_PROVIDER_ID
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
