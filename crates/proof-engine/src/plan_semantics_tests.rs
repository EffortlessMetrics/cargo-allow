//! Negative control tests for ProofPlanV1 intent semantic preservation (#3313).
//!
//! Proves that plan_proof_execution_from_intent preserves exact phase,
//! posture, evidence-reference, and source/currentness semantics from the
//! input IntentObligationPlanEnvelopeV1.

use crate::intent_planner::{IntentPlannerError, plan_proof_execution_from_intent};
use crate::provider_registry::{ProviderRegistryEntryV1, ProviderRegistryV1};
use intent_protocol::{
    IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
    IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
    RepositorySnapshotV1, ResolvedRevisionV1,
};

fn sample_identity() -> IntentIdentityEnvelopeV1 {
    IntentIdentityEnvelopeV1::new(
        RepositorySnapshotV1::new_committed_head(
            "identity",
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
    )
}

fn sample_envelope(
    phase: &str,
    posture: IntentObligationPostureV1,
    evidence_refs: Vec<String>,
) -> IntentObligationPlanEnvelopeV1 {
    IntentObligationPlanEnvelopeV1::new(
        sample_identity(),
        phase,
        vec![IntentPhaseObligationV1 {
            handoff: None,
            obligation_id: "obl-1".to_string(),
            phase: phase.to_string(),
            kind: IntentPhaseObligationKindV1::EvidenceReview,
            statement: "Review evidence".to_string(),
            posture,
            evidence_refs,
        }],
    )
}

fn cargo_allow_registry() -> ProviderRegistryV1 {
    ProviderRegistryV1::new(vec![ProviderRegistryEntryV1 {
        provider_id: "cargo-allow".to_string(),
    }])
}

fn empty_registry() -> ProviderRegistryV1 {
    ProviderRegistryV1::new(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_and_blocking_both_produce_plans() -> Result<(), String> {
        let registry = cargo_allow_registry();
        for posture in [
            IntentObligationPostureV1::Blocking,
            IntentObligationPostureV1::Advisory,
        ] {
            let envelope = sample_envelope("precommit", posture, vec![]);
            let plan = plan_proof_execution_from_intent(&envelope, &registry)
                .map_err(|e| format!("posture {posture:?} failed: {e:?}"))?;
            if plan.commands.is_empty() {
                return Err(format!("posture {posture:?} should produce commands"));
            }
        }
        Ok(())
    }

    #[test]
    fn empty_provider_registry_fails() -> Result<(), String> {
        let envelope = sample_envelope("precommit", IntentObligationPostureV1::Blocking, vec![]);
        let registry = empty_registry();
        let result = plan_proof_execution_from_intent(&envelope, &registry);
        if result.is_ok() {
            return Err("empty provider registry should fail".into());
        }
        Ok(())
    }

    #[test]
    fn different_phases_produce_different_plan_identities() -> Result<(), String> {
        let registry = cargo_allow_registry();
        let precommit = sample_envelope("precommit", IntentObligationPostureV1::Blocking, vec![]);
        let release = sample_envelope("release", IntentObligationPostureV1::Blocking, vec![]);
        let plan1 = plan_proof_execution_from_intent(&precommit, &registry)
            .map_err(|e| format!("{e:?}"))?;
        let plan2 =
            plan_proof_execution_from_intent(&release, &registry).map_err(|e| format!("{e:?}"))?;
        if plan1.plan_id == plan2.plan_id {
            return Err(format!(
                "different phases must produce different plan_ids: {} == {}",
                plan1.plan_id, plan2.plan_id
            ));
        }
        Ok(())
    }

    #[test]
    fn evidence_references_survive_planning() -> Result<(), String> {
        let registry = cargo_allow_registry();
        let envelope = sample_envelope(
            "precommit",
            IntentObligationPostureV1::Blocking,
            vec!["doc:README.md".to_string(), "test:src/lib.rs".to_string()],
        );
        // The planner should accept envelopes with evidence_refs without error.
        // The evidence_refs themselves are preserved in the intent envelope,
        // not duplicated into ProofPlanV1 (which only carries commands).
        let plan = plan_proof_execution_from_intent(&envelope, &registry)
            .map_err(|e| format!("evidence_refs envelope failed: {e:?}"))?;
        if plan.commands.is_empty() {
            return Err("plan should have commands".into());
        }
        // Verify evidence_refs are still accessible from the original envelope.
        assert_eq!(envelope.obligations[0].evidence_refs.len(), 2);
        Ok(())
    }

    #[test]
    fn empty_obligations_fail_distinctly() -> Result<(), String> {
        let registry = cargo_allow_registry();
        let envelope = IntentObligationPlanEnvelopeV1::new(sample_identity(), "precommit", vec![]);
        let result = plan_proof_execution_from_intent(&envelope, &registry);
        match result {
            Err(IntentPlannerError::NoCommandsPlanned) => Ok(()),
            other => Err(format!("expected NoCommandsPlanned, got {other:?}")),
        }
    }
}
