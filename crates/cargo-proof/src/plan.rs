//! Proof plan command wired to proof-engine (#2589-B).
//!
//! The plan CLI now consumes intent-protocol obligation input via the intent
//! planner entry point (#3312). Historical ChangeObligationPlanV1 fixtures
//! are routed through the compatibility converter from slice 2 (#3311) so no
//! fixture silently bypasses the real cargo-intent handoff.

use std::path::Path;

use proof_engine::FakeProofProviderV1;
use proof_engine::{
    ProviderRegistryV1, convert_legacy_obligation_plan, load_obligation_plan_toml,
    plan_proof_execution_from_intent, register_validated_provider,
};
use proof_protocol::{PROOF_PLAN_SCHEMA_ID, ProofPlanV1};

use crate::render::{OutputFormat, PlanFrameV1, emit_frame};

pub const PLAN_FRAME_SCHEMA_ID: &str = "cargo-proof.plan-frame.v1";
pub const PLAN_CLAIM_BOUNDARY: &str =
    "Obligation-to-proof-plan projection only; process execution remains caller-owned.";

/// Plan proof execution from an obligation plan file.
///
/// Accepts both legacy ChangeObligationPlanV1 TOML and intent-protocol
/// JSON formats. Legacy fixtures are routed through the compatibility
/// converter (#3311) so no fixture bypasses the intent handoff.
pub fn plan_from_obligation_path(path: &Path) -> Result<ProofPlanV1, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;

    // Try legacy ChangeObligationPlanV1 TOML first (backward compatibility).
    // If successful, route through the compatibility converter (#3311).
    if let Ok(legacy) = load_obligation_plan_toml(&text) {
        let envelope = convert_legacy_obligation_plan(&legacy)
            .map_err(|err| format!("legacy conversion failed: {}", err.as_str()))?;
        return plan_from_intent_envelope(&envelope);
    }

    // Fall back to intent-protocol JSON input.
    let envelope: intent_protocol::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&text).map_err(|err| format!("parse intent envelope JSON: {err}"))?;
    plan_from_intent_envelope(&envelope)
}

/// Plan proof execution from an intent obligation plan envelope.
fn plan_from_intent_envelope(
    envelope: &intent_protocol::IntentObligationPlanEnvelopeV1,
) -> Result<ProofPlanV1, String> {
    let mut registry = ProviderRegistryV1::new(Vec::new());
    register_validated_provider(&mut registry, &FakeProofProviderV1::with_id("cargo-allow"))
        .map_err(|err| err.as_str().to_string())?;
    plan_proof_execution_from_intent(envelope, &registry).map_err(|err| err.as_str().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_TOML: &str = r#"
schema_id = "proof.change-obligation-plan.v1"
plan_id = "test-legacy-plan"
obligations = [
  { obligation_id = "obl-1", provider_id = "fake", proof_kind = "evidence_review" },
]
"#;

    #[test]
    fn legacy_toml_routes_through_compatibility_conversion() -> Result<(), String> {
        let temp = std::env::temp_dir().join("cargo-proof-3312-legacy.toml");
        std::fs::write(&temp, LEGACY_TOML).map_err(|e| format!("write: {e}"))?;
        let plan = plan_from_obligation_path(&temp)?;
        if plan.commands.is_empty() {
            return Err("legacy TOML should produce at least one command".into());
        }
        std::fs::remove_file(&temp).ok();
        Ok(())
    }

    #[test]
    fn intent_json_input_works_directly() -> Result<(), String> {
        // Build the envelope programmatically rather than from JSON, since
        // the snapshot has fields that are hard to get right in hand-written JSON.
        use intent_protocol::{
            IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPostureV1,
            IntentPhaseObligationKindV1, IntentPhaseObligationV1, RepositorySnapshotV1,
            ResolvedRevisionV1,
        };
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
        let envelope = intent_protocol::IntentObligationPlanEnvelopeV1::new(
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
        let plan = plan_from_intent_envelope(&envelope)?;
        if plan.commands.is_empty() {
            return Err("intent envelope should produce at least one command".into());
        }
        Ok(())
    }
}
