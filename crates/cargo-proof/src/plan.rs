//! Proof plan command wired to proof-engine (#2589-B).
//!
//! The plan CLI consumes intent-protocol obligation input via the intent
//! planner entry point (#3310/#3312). The legacy ChangeObligationPlanV1
//! path has been deleted (#3314).

use std::path::Path;

use proof_engine::FakeProofProviderV1;
use proof_engine::{
    ProviderRegistryV1, intent_obligation_plan_digest, plan_proof_execution_from_intent,
    register_validated_provider,
};
use proof_protocol::{PROOF_PLAN_SCHEMA_ID, ProofPlanV1};

use crate::render::{OutputFormat, PlanFrameV1, emit_frame};

pub const PLAN_FRAME_SCHEMA_ID: &str = "cargo-proof.plan-frame.v1";
pub const PLAN_CLAIM_BOUNDARY: &str =
    "Obligation-to-proof-plan projection only; process execution remains caller-owned.";

/// Plan outcome binding the exact intent plan identity (#3316).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOutcomeV1 {
    pub plan: ProofPlanV1,
    pub intent_plan_digest: String,
}

/// Plan proof execution from an intent obligation plan file (JSON).
pub fn plan_from_obligation_path(path: &Path) -> Result<PlanOutcomeV1, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let envelope: intent_protocol::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&text).map_err(|err| format!("parse intent envelope JSON: {err}"))?;
    plan_from_intent_envelope(&envelope)
}

/// Plan proof execution from an intent obligation plan envelope.
fn plan_from_intent_envelope(
    envelope: &intent_protocol::IntentObligationPlanEnvelopeV1,
) -> Result<PlanOutcomeV1, String> {
    let mut registry = ProviderRegistryV1::new(Vec::new());
    register_validated_provider(&mut registry, &FakeProofProviderV1::with_id("cargo-allow"))
        .map_err(|err| err.as_str().to_string())?;
    let plan = plan_proof_execution_from_intent(envelope, &registry)
        .map_err(|err| err.as_str().to_string())?;
    let intent_plan_digest = intent_obligation_plan_digest(envelope)?;
    Ok(PlanOutcomeV1 {
        plan,
        intent_plan_digest,
    })
}

pub fn render_plan_frame(outcome: &PlanOutcomeV1, format: OutputFormat) -> Result<String, String> {
    let frame = PlanFrameV1 {
        schema_id: PLAN_FRAME_SCHEMA_ID.to_string(),
        plan_id: outcome.plan.plan_id.clone(),
        intent_plan_digest: outcome.intent_plan_digest.clone(),
        command_count: outcome.plan.commands.len(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
        IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
        RepositorySnapshotV1, ResolvedRevisionV1,
    };

    #[test]
    fn intent_json_input_works() -> Result<(), String> {
        let identity = IntentIdentityEnvelopeV1::new(
            RepositorySnapshotV1::new_committed_head(
                "test",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "abc".to_string(),
                    tree: String::new(),
                },
            ),
            IntentArtifactKindV1::RequirementDocument,
            "test-artifact",
            "test/source.md",
            "test-content",
        );
        let envelope = IntentObligationPlanEnvelopeV1::new(
            identity,
            "precommit",
            vec![IntentPhaseObligationV1 {
                obligation_id: "obl-direct".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec![],
            }],
        );
        let outcome = plan_from_intent_envelope(&envelope)?;
        if outcome.plan.commands.is_empty() {
            return Err("intent envelope should produce at least one command".into());
        }
        if !outcome.intent_plan_digest.starts_with("sha256:v1:") {
            return Err(format!(
                "plan outcome must bind the intent digest: {}",
                outcome.intent_plan_digest
            ));
        }
        if !outcome.plan.plan_id.contains(&outcome.intent_plan_digest) {
            return Err("plan identity must embed the intent plan digest".into());
        }
        Ok(())
    }
}
