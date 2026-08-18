//! Proof plan command wired to proof-engine (#2589-B).
//!
//! The plan CLI consumes intent-protocol obligation input via the intent
//! planner entry point (#3310/#3312). The legacy proof-owned obligation
//! path has been deleted (#3314) and is guarded against reintroduction
//! (#3317).

use std::path::Path;

use proof_engine::{
    IntentPlannerError, ProviderRegistryV1, intent_obligation_plan_digest,
    plan_proof_execution_from_intent,
};

/// The provider this product intends to select once the feature-gated
/// registry lands (#2938). Named in every unavailable result so output
/// states exactly what is missing; never constructed as a fake.
pub const INTENDED_PROVIDER_ID: &str = "cargo-allow";
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
    // Provider selection is not yet established (#3598/#2938): the
    // product registry is empty and no provider is fabricated. The
    // intent digest stays available, and the failure names the exact
    // missing provider and the limitation.
    let registry = ProviderRegistryV1::new(Vec::new());
    // Digest validation still runs first: an invalid envelope fails as a
    // usage error rather than a provider result.
    intent_obligation_plan_digest(envelope)?;
    let Err(err) = plan_proof_execution_from_intent(envelope, &registry) else {
        return Err(
            "planner produced a plan from an empty provider registry; provider selection must not fabricate"
                .to_string(),
        );
    };
    Err(format!(
        "provider unavailable: executable provider selection is not yet established;          intended provider `{INTENDED_PROVIDER_ID}` is not registered and no provider is fabricated;          planner result: {}",
        planner_result_detail(&err),
    ))
}

fn planner_result_detail(err: &IntentPlannerError) -> String {
    match err {
        IntentPlannerError::ProviderRegistry(detail) => {
            format!("{} ({})", err.as_str(), detail.as_str())
        }
        other => other.as_str().to_string(),
    }
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
    fn empty_registry_plan_fails_explicitly() -> Result<(), String> {
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
        let Err(message) = plan_from_intent_envelope(&envelope) else {
            return Err("empty registry must not produce a plan".into());
        };
        for required in [
            "provider unavailable",
            INTENDED_PROVIDER_ID,
            "not yet established",
            "no provider is fabricated",
        ] {
            if !message.contains(required) {
                return Err(format!(
                    "unavailable result missing {required:?}: {message}"
                ));
            }
        }
        Ok(())
    }
}
