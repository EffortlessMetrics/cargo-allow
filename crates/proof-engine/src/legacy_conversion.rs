//! Compatibility-only conversion from ChangeObligationPlanV1 to
//! IntentObligationPlanEnvelopeV1 (#3311 / #2936 slice 2).
//!
//! This module provides a transitional conversion function that maps the
//! legacy proof-owned obligation plan into the canonical intent-protocol
//! envelope. It is compatibility-only: once the proof CLI and fixtures
//! migrate to the intent format directly (#3312), this converter is deleted
//! (#3314).

use crate::obligation_plan::{ChangeObligationPlanV1, ChangeObligationV1};
use intent_protocol::{
    IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
    IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
    RepositorySnapshotV1, ResolvedRevisionV1,
};

/// Conversion error for legacy obligation plan to intent envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyConversionError {
    EmptyObligations,
    EmptyPhase,
    EmptyObligationId { index: usize },
}

impl LegacyConversionError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptyObligations => "empty_obligations",
            Self::EmptyPhase => "empty_phase",
            Self::EmptyObligationId { .. } => "empty_obligation_id",
        }
    }
}

/// Convert a legacy ChangeObligationPlanV1 into an IntentObligationPlanEnvelopeV1.
///
/// The legacy plan does not carry phase, posture, or evidence_refs. The
/// converter fills defaults:
/// - phase: "precommit" (the only phase the legacy planner supported)
/// - posture: Blocking (legacy obligations were always blocking)
/// - kind: EvidenceReview (the legacy proof_kind maps to evidence review)
/// - statement: empty string (legacy plans did not carry statements)
/// - evidence_refs: empty vec
///
/// The plan_id from the legacy plan becomes part of the identity envelope's
/// content_identity so different plans produce different envelopes.
pub fn convert_legacy_obligation_plan(
    legacy: &ChangeObligationPlanV1,
) -> Result<IntentObligationPlanEnvelopeV1, LegacyConversionError> {
    if legacy.obligations.is_empty() {
        return Err(LegacyConversionError::EmptyObligations);
    }

    let identity = IntentIdentityEnvelopeV1::new(
        RepositorySnapshotV1::new_committed_head(
            "legacy-conversion",
            "sha1",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "converted".to_string(),
                tree: String::new(),
            },
        ),
        IntentArtifactKindV1::RequirementDocument,
        "legacy-obligation-plan",
        "proof/change-obligation-plan",
        &legacy.plan_id,
    );

    let obligations = legacy
        .obligations
        .iter()
        .enumerate()
        .map(|(index, obligation)| convert_obligation(index, obligation))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(IntentObligationPlanEnvelopeV1::new(
        identity,
        "precommit",
        obligations,
    ))
}

fn convert_obligation(
    index: usize,
    obligation: &ChangeObligationV1,
) -> Result<IntentPhaseObligationV1, LegacyConversionError> {
    if obligation.obligation_id.is_empty() {
        return Err(LegacyConversionError::EmptyObligationId { index });
    }
    Ok(IntentPhaseObligationV1 {
        obligation_id: obligation.obligation_id.clone(),
        phase: "precommit".to_string(),
        kind: IntentPhaseObligationKindV1::EvidenceReview,
        statement: String::new(),
        posture: IntentObligationPostureV1::Blocking,
        evidence_refs: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obligation_plan::{ChangeObligationPlanV1, ChangeObligationV1};

    fn sample_legacy_plan() -> ChangeObligationPlanV1 {
        ChangeObligationPlanV1::new(
            "test-plan-001",
            vec![
                ChangeObligationV1 {
                    obligation_id: "obl-check-no-new".to_string(),
                    provider_id: "cargo-allow".to_string(),
                    proof_kind: "evidence_review".to_string(),
                },
                ChangeObligationV1 {
                    obligation_id: "obl-runtime-promotion".to_string(),
                    provider_id: "cargo-allow".to_string(),
                    proof_kind: "evidence_review".to_string(),
                },
            ],
        )
    }

    #[test]
    fn converts_legacy_plan_to_intent_envelope() -> Result<(), String> {
        let legacy = sample_legacy_plan();
        let envelope = convert_legacy_obligation_plan(&legacy)
            .map_err(|e| format!("conversion failed: {e:?}"))?;
        assert_eq!(envelope.obligations.len(), 2);
        assert_eq!(envelope.phase, "precommit");
        assert_eq!(envelope.obligations[0].obligation_id, "obl-check-no-new");
        assert_eq!(
            envelope.obligations[0].posture,
            IntentObligationPostureV1::Blocking
        );
        assert_eq!(
            envelope.obligations[0].kind,
            IntentPhaseObligationKindV1::EvidenceReview
        );
        Ok(())
    }

    #[test]
    fn different_plan_ids_produce_different_envelopes() -> Result<(), String> {
        let plan1 = sample_legacy_plan();
        let mut plan2 = sample_legacy_plan();
        plan2.plan_id = "test-plan-002".to_string();
        let env1 = convert_legacy_obligation_plan(&plan1).map_err(|e| format!("{e:?}"))?;
        let env2 = convert_legacy_obligation_plan(&plan2).map_err(|e| format!("{e:?}"))?;
        if env1.identity.content_identity == env2.identity.content_identity {
            return Err("different plan_ids should produce different content identities".into());
        }
        Ok(())
    }

    #[test]
    fn empty_obligations_fail() -> Result<(), String> {
        let legacy = ChangeObligationPlanV1::new("empty", vec![]);
        let result = convert_legacy_obligation_plan(&legacy);
        if result.is_ok() {
            return Err("empty obligations should fail".into());
        }
        assert_eq!(result.unwrap_err(), LegacyConversionError::EmptyObligations);
        Ok(())
    }

    #[test]
    fn empty_obligation_id_fails() -> Result<(), String> {
        let legacy = ChangeObligationPlanV1::new(
            "test",
            vec![ChangeObligationV1 {
                obligation_id: String::new(),
                provider_id: "cargo-allow".to_string(),
                proof_kind: "evidence_review".to_string(),
            }],
        );
        let result = convert_legacy_obligation_plan(&legacy);
        assert!(matches!(
            result,
            Err(LegacyConversionError::EmptyObligationId { index: 0 })
        ));
        Ok(())
    }

    #[test]
    fn converted_envelope_works_with_intent_planner() -> Result<(), String> {
        use crate::intent_planner::plan_proof_execution_from_intent;
        use crate::provider_registry::{ProviderRegistryEntryV1, ProviderRegistryV1};

        let legacy = sample_legacy_plan();
        let envelope = convert_legacy_obligation_plan(&legacy)
            .map_err(|e| format!("conversion failed: {e:?}"))?;
        let registry = ProviderRegistryV1::new(vec![ProviderRegistryEntryV1 {
            provider_id: "cargo-allow".to_string(),
        }]);
        let plan = plan_proof_execution_from_intent(&envelope, &registry)
            .map_err(|e| format!("planning failed: {e:?}"))?;
        assert!(!plan.commands.is_empty());
        Ok(())
    }
}
