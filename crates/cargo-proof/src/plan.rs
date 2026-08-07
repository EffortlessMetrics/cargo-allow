//! Proof plan command wired to proof-engine (#2589-B).

use std::path::Path;

use proof_engine::FakeProofProviderV1;
use proof_engine::{
    ProviderRegistryV1, load_obligation_plan_toml, plan_proof_execution,
    register_validated_provider,
};
use proof_protocol::{PROOF_PLAN_SCHEMA_ID, ProofPlanV1};

use crate::render::{OutputFormat, PlanFrameV1, emit_frame};

pub const PLAN_FRAME_SCHEMA_ID: &str = "cargo-proof.plan-frame.v1";
pub const PLAN_CLAIM_BOUNDARY: &str =
    "Obligation-to-proof-plan projection only; process execution remains caller-owned.";

pub fn plan_from_obligation_path(path: &Path) -> Result<ProofPlanV1, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let obligation = load_obligation_plan_toml(&text)?;
    let mut registry = ProviderRegistryV1::new(Vec::new());
    register_validated_provider(&mut registry, &FakeProofProviderV1::new())
        .map_err(|err| err.as_str().to_string())?;
    plan_proof_execution(&obligation, &registry).map_err(|err| err.as_str().to_string())
}

pub fn render_plan_frame(plan: &ProofPlanV1, format: OutputFormat) -> Result<String, String> {
    let frame = PlanFrameV1 {
        schema_id: PLAN_FRAME_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        command_count: plan.commands.len(),
        claim_boundary: PLAN_CLAIM_BOUNDARY.to_string(),
    };
    let rendered = emit_frame(&frame, format)?;
    if format == OutputFormat::Json {
        return Ok(rendered);
    }
    let mut output = rendered;
    output.push_str(&format!("schema: {PROOF_PLAN_SCHEMA_ID}\n"));
    Ok(output)
}
