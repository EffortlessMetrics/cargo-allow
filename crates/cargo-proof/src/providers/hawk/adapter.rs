//! Hawk ProofProviderV1 implementation (#2555).

use proof_engine::{ProofProviderV1, ProviderApiError};
use proof_protocol::{
    ProofCapabilityCatalogV1, ProofCapabilityKindV1, ProofCapabilityV1, ProofPlanV1,
};

pub const HAWK_PROOF_PROVIDER_ID: &str = "proof.hawk.v1";

pub struct HawkProofProviderV1 {
    catalog: ProofCapabilityCatalogV1,
}

impl HawkProofProviderV1 {
    pub fn new() -> Self {
        Self {
            catalog: ProofCapabilityCatalogV1::new(
                HAWK_PROOF_PROVIDER_ID,
                vec![
                    ProofCapabilityV1 {
                        capability_id: "hawk.analysis-receipt.validate".to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "cargo-hawk".to_string(),
                        statement: "Validate captured Hawk JSON report".to_string(),
                    },
                    ProofCapabilityV1 {
                        capability_id: "hawk.finding.map".to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "cargo-hawk".to_string(),
                        statement: "Map Hawk findings to adapter result classes".to_string(),
                    },
                    ProofCapabilityV1 {
                        capability_id: "hawk.source-anchor.resolve".to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "cargo-hawk".to_string(),
                        statement: "Resolve intent source anchors to Hawk declaration identities"
                            .to_string(),
                    },
                ],
            ),
        }
    }
}

impl Default for HawkProofProviderV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofProviderV1 for HawkProofProviderV1 {
    fn provider_id(&self) -> &str {
        HAWK_PROOF_PROVIDER_ID
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
