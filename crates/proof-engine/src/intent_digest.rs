//! Load-bearing intent obligation-plan digest (#3316 / #2936 slice 6).
//!
//! The digest covers the complete intent obligation plan content: phase,
//! every obligation (id, phase, kind, statement, posture, evidence refs),
//! and the intent identity envelope (repository snapshot, artifact identity,
//! source identity). It is embedded in the proof plan identity, the proof
//! cache key, and the currentness binding, so a changed or stale intent plan
//! always yields a distinct (non-current) proof identity.

use intent_protocol::IntentObligationPlanEnvelopeV1;
use sha2::{Digest, Sha256};

pub const INTENT_PLAN_IDENTITY_PREFIX: &str = "intent-proof-plan";

/// Versioned SHA-256 digest over the canonical serialization of an intent
/// obligation plan envelope.
///
/// `serde_json` serialization of the envelope is deterministic: struct field
/// order is fixed by the type definitions and no map fields participate.
pub fn intent_obligation_plan_digest(
    envelope: &IntentObligationPlanEnvelopeV1,
) -> Result<String, String> {
    let canonical = serde_json::to_string(envelope)
        .map_err(|err| format!("serialize intent envelope: {err}"))?;
    let digest = Sha256::digest(canonical.as_bytes());
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:v1:{hex}"))
}

/// Durable proof plan identity derived from the intent plan digest.
///
/// Any change to the intent plan content (phase, obligations, evidence
/// references, source identity) produces a different plan identity.
pub fn intent_plan_identity(envelope: &IntentObligationPlanEnvelopeV1) -> Result<String, String> {
    Ok(format!(
        "{INTENT_PLAN_IDENTITY_PREFIX}:{}",
        intent_obligation_plan_digest(envelope)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn sample_envelope() -> IntentObligationPlanEnvelopeV1 {
        IntentObligationPlanEnvelopeV1::new(
            sample_identity(),
            "precommit",
            vec![IntentPhaseObligationV1 {
                handoff: None,
                obligation_id: "obl-1".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec!["doc:README.md".to_string()],
            }],
        )
    }

    #[test]
    fn digest_is_deterministic() -> Result<(), String> {
        let envelope = sample_envelope();
        let first = intent_obligation_plan_digest(&envelope)?;
        let second = intent_obligation_plan_digest(&envelope)?;
        if first != second || !first.starts_with("sha256:v1:") {
            return Err(format!(
                "digest must be deterministic and versioned: {first}"
            ));
        }
        Ok(())
    }

    #[test]
    fn evidence_reference_change_changes_digest() -> Result<(), String> {
        let mut changed = sample_envelope();
        if let Some(first) = changed.obligations.first_mut() {
            first.evidence_refs.push("doc:NEW.md".to_string());
        }
        if intent_obligation_plan_digest(&sample_envelope())?
            == intent_obligation_plan_digest(&changed)?
        {
            return Err("evidence reference change must change the digest".into());
        }
        Ok(())
    }

    #[test]
    fn source_identity_change_changes_digest() -> Result<(), String> {
        let mut identity = sample_identity();
        identity.content_identity = "changed-content".to_string();
        let changed = IntentObligationPlanEnvelopeV1::new(
            identity,
            "precommit",
            sample_envelope().obligations.clone(),
        );
        if intent_obligation_plan_digest(&sample_envelope())?
            == intent_obligation_plan_digest(&changed)?
        {
            return Err("source identity change must change the digest".into());
        }
        Ok(())
    }

    #[test]
    fn statement_change_changes_plan_identity() -> Result<(), String> {
        let mut changed = sample_envelope();
        if let Some(first) = changed.obligations.first_mut() {
            first.statement = "Different statement".to_string();
        }
        if intent_plan_identity(&sample_envelope())? == intent_plan_identity(&changed)? {
            return Err("statement change must change the plan identity".into());
        }
        Ok(())
    }

    #[test]
    fn identity_embeds_digest() -> Result<(), String> {
        let envelope = sample_envelope();
        let digest = intent_obligation_plan_digest(&envelope)?;
        let identity = intent_plan_identity(&envelope)?;
        if !identity.starts_with("intent-proof-plan:sha256:v1:") {
            return Err(format!("plan identity must embed the digest: {identity}"));
        }
        if !identity.contains(&digest) {
            return Err(format!(
                "plan identity must contain the digest {digest}: {identity}"
            ));
        }
        Ok(())
    }
}
