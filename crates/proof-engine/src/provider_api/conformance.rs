//! Provider conformance harness (#2603-A).

use proof_protocol::{ProofPlanCommandV1, ProofPlanV1};

use super::contracts::{ProofProviderV1, validate_provider_plan};
use super::fake_provider::FakeProofProviderV1;

pub const CONFORMANCE_SCENARIO_ID: &str = "proof-provider-api-conformance-v1";

pub fn run_fake_provider_conformance() -> Result<(), String> {
    let provider = FakeProofProviderV1::new();
    run_provider_conformance(&provider)
}

pub fn run_provider_conformance(provider: &dyn ProofProviderV1) -> Result<(), String> {
    let plan = ProofPlanV1::new(
        CONFORMANCE_SCENARIO_ID,
        vec![ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )],
    );
    validate_provider_plan(provider, &plan).map_err(|err| err.as_str().to_string())
}
