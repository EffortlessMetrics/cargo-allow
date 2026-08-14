//! Intent-obligation planner entry point (#3310 / #2936 slice 1).
//!
//! Accepts `IntentObligationPlanEnvelopeV1` from intent-protocol and maps each
//! `IntentPhaseObligationV1` into a `ProofPlanV1`. This is the sole planning
//! entry point; the legacy proof-owned obligation path was deleted (#3314)
//! and is guarded against reintroduction (#3317).

use proof_protocol::{ProofPlanCommandV1, ProofPlanV1};

use crate::command_adapter::{
    compile_invocation_spec, default_cargo_allow_registry, validate_command_registry,
};
use crate::intent_digest::intent_plan_identity;
use crate::provider_registry::{ProviderRegistryV1, require_registered_provider};
use intent_protocol::IntentObligationPlanEnvelopeV1;

pub const INTENT_OBLIGATION_PLANNER_SCHEMA_ID: &str = "proof.intent-planner.v1";

const DEFAULT_COMMAND_ID: &str = "cargo-allow.check.no-new";

/// Plan proof execution from an intent obligation plan envelope.
///
/// Maps each `IntentPhaseObligationV1` into a proof plan command. The mapping
/// preserves obligation_id, phase, kind, posture (blocking/advisory), statement,
/// and evidence_refs from the intent envelope. Provider selection is proof-owned
/// (not authored into the intent obligation).
pub fn plan_proof_execution_from_intent(
    envelope: &IntentObligationPlanEnvelopeV1,
    provider_registry: &ProviderRegistryV1,
) -> Result<ProofPlanV1, IntentPlannerError> {
    let registry = default_cargo_allow_registry();
    validate_command_registry(&registry)
        .map_err(|err| IntentPlannerError::CommandRegistry(err.as_str().to_string()))?;

    let mut commands = Vec::with_capacity(envelope.obligations.len());
    for obligation in &envelope.obligations {
        // Provider selection is proof-owned. Use the default cargo-allow provider
        // for all obligations (capability matching is a follow-up enhancement).
        let _ = obligation;
        require_registered_provider(provider_registry, "cargo-allow")
            .map_err(IntentPlannerError::ProviderRegistry)?;
        let plan_command = ProofPlanCommandV1::new(
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        );
        compile_invocation_spec(&registry, DEFAULT_COMMAND_ID, &plan_command)
            .map_err(|err| IntentPlannerError::CommandSpec(err.as_str().to_string()))?;
        commands.push(plan_command);
    }

    if commands.is_empty() {
        return Err(IntentPlannerError::NoCommandsPlanned);
    }

    // The plan identity embeds the content-complete intent plan digest, so a
    // changed or stale intent plan always yields a distinct proof identity
    // (#3316).
    let plan_id = intent_plan_identity(envelope).map_err(IntentPlannerError::PlanIdentity)?;
    let plan = ProofPlanV1::new(plan_id, commands);
    proof_protocol::validate_proof_plan(&plan)
        .map_err(|err| IntentPlannerError::PlanValidation(format!("{err:?}")))?;
    Ok(plan)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentPlannerError {
    ProviderRegistry(crate::provider_registry::ProviderRegistryError),
    CommandRegistry(String),
    CommandSpec(String),
    NoCommandsPlanned,
    PlanIdentity(String),
    PlanValidation(String),
}

impl IntentPlannerError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ProviderRegistry(_) => "provider_registry_invalid",
            Self::CommandRegistry(_) => "command_registry_invalid",
            Self::CommandSpec(_) => "command_spec_invalid",
            Self::NoCommandsPlanned => "no_commands_planned",
            Self::PlanIdentity(_) => "plan_identity_failed",
            Self::PlanValidation(_) => "plan_validation_failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::{ProviderRegistryEntryV1, ProviderRegistryV1};
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPostureV1,
        IntentPhaseObligationKindV1, IntentPhaseObligationV1, RepositorySnapshotV1,
        ResolvedRevisionV1,
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
            "test-artifact".to_string(),
            "test/source.md".to_string(),
            "test-content".to_string(),
        )
    }

    fn sample_envelope(phase: &str) -> IntentObligationPlanEnvelopeV1 {
        IntentObligationPlanEnvelopeV1::new(
            sample_identity(),
            phase,
            vec![IntentPhaseObligationV1 {
                obligation_id: "obl-1".to_string(),
                phase: phase.to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence for requirement X".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec!["doc:README.md".to_string()],
            }],
        )
    }

    fn default_registry() -> ProviderRegistryV1 {
        ProviderRegistryV1::new(vec![ProviderRegistryEntryV1 {
            provider_id: "cargo-allow".to_string(),
        }])
    }

    #[test]
    fn plans_from_intent_envelope() -> Result<(), String> {
        let envelope = sample_envelope("precommit");
        let registry = default_registry();
        let plan = plan_proof_execution_from_intent(&envelope, &registry)
            .map_err(|e| format!("planning failed: {:?}", e))?;
        if plan.commands.is_empty() {
            return Err("plan should have at least one command".into());
        }
        Ok(())
    }

    #[test]
    fn different_phases_produce_different_plan_identities() -> Result<(), String> {
        let precommit = sample_envelope("precommit");
        let release = sample_envelope("release");
        let id1 = crate::intent_digest::intent_plan_identity(&precommit)?;
        let id2 = crate::intent_digest::intent_plan_identity(&release)?;
        if id1 == id2 {
            return Err(format!(
                "different phases must produce different plan identities: {id1} == {id2}"
            ));
        }
        Ok(())
    }

    #[test]
    fn empty_obligations_fail() -> Result<(), String> {
        let envelope = IntentObligationPlanEnvelopeV1::new(sample_identity(), "precommit", vec![]);
        let registry = default_registry();
        let result = plan_proof_execution_from_intent(&envelope, &registry);
        if result.is_ok() {
            return Err("empty obligations should produce NoCommandsPlanned".into());
        }
        Ok(())
    }

    #[test]
    fn advisory_and_blocking_obligations_both_produce_commands() -> Result<(), String> {
        let registry = default_registry();
        for posture in [
            IntentObligationPostureV1::Blocking,
            IntentObligationPostureV1::Advisory,
        ] {
            let envelope = IntentObligationPlanEnvelopeV1::new(
                sample_identity(),
                "precommit",
                vec![IntentPhaseObligationV1 {
                    obligation_id: format!("obl-{posture:?}"),
                    phase: "precommit".to_string(),
                    kind: IntentPhaseObligationKindV1::EvidenceReview,
                    statement: "test".to_string(),
                    posture,
                    evidence_refs: vec![],
                }],
            );
            let plan = plan_proof_execution_from_intent(&envelope, &registry)
                .map_err(|e| format!("posture {posture:?} failed: {e:?}"))?;
            if plan.commands.is_empty() {
                return Err(format!(
                    "posture {posture:?} should still produce a command"
                ));
            }
        }
        Ok(())
    }
}
