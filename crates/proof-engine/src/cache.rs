//! Proof receipt cache for proof-engine orchestration (#2589-A).

use intent_protocol::IntentObligationPlanEnvelopeV1;
use proof_protocol::{ProofReceiptSetV1, validate_receipt_set};

use crate::intent_digest::intent_obligation_plan_digest;

pub const PROOF_CACHE_SCHEMA_ID: &str = "proof.cache.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheEntryV1 {
    pub cache_key: String,
    pub receipt_set: ProofReceiptSetV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheV1 {
    pub schema_id: String,
    pub entries: Vec<ProofCacheEntryV1>,
}

impl ProofCacheV1 {
    pub fn new() -> Self {
        Self {
            schema_id: PROOF_CACHE_SCHEMA_ID.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn insert(
        &mut self,
        cache_key: impl Into<String>,
        receipt_set: ProofReceiptSetV1,
    ) -> Result<(), CacheError> {
        validate_receipt_set(&receipt_set).map_err(CacheError::Receipt)?;
        let cache_key = cache_key.into();
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.cache_key == cache_key)
        {
            existing.receipt_set = receipt_set;
            return Ok(());
        }
        self.entries.push(ProofCacheEntryV1 {
            cache_key,
            receipt_set,
        });
        Ok(())
    }

    pub fn get(&self, cache_key: &str) -> Option<&ProofReceiptSetV1> {
        self.entries
            .iter()
            .find(|entry| entry.cache_key == cache_key)
            .map(|entry| &entry.receipt_set)
    }
}

impl Default for ProofCacheV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    InvalidSchemaId { observed: String },
    Receipt(proof_protocol::ProofReceiptError),
    IntentDigest(String),
}

impl CacheError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::Receipt(_) => "receipt_invalid",
            Self::IntentDigest(_) => "intent_digest_failed",
        }
    }
}

pub fn validate_proof_cache(cache: &ProofCacheV1) -> Result<(), CacheError> {
    if cache.schema_id != PROOF_CACHE_SCHEMA_ID {
        return Err(CacheError::InvalidSchemaId {
            observed: cache.schema_id.clone(),
        });
    }
    for entry in &cache.entries {
        validate_receipt_set(&entry.receipt_set).map_err(CacheError::Receipt)?;
    }
    Ok(())
}

pub fn cache_key_for_plan(plan_id: &str, digest: &str) -> String {
    format!("{plan_id}::{digest}")
}

/// Cache key binding the exact intent plan identity (#3316).
///
/// The key embeds the content-complete intent obligation-plan digest, so a
/// changed intent plan (phase, obligations, evidence references, source
/// identity) always resolves to a distinct cache identity and cannot reuse
/// receipts captured for a different intent plan.
pub fn cache_key_for_intent_plan(
    envelope: &IntentObligationPlanEnvelopeV1,
) -> Result<String, CacheError> {
    let digest = intent_obligation_plan_digest(envelope).map_err(CacheError::IntentDigest)?;
    Ok(format!("intent-plan::{digest}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPostureV1,
        IntentPhaseObligationKindV1, IntentPhaseObligationV1, RepositorySnapshotV1,
        ResolvedRevisionV1,
    };
    use proof_protocol::{ProofReceiptBindingV1, ProofReceiptSetV1};

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

    fn receipt_set(plan_id: &str) -> ProofReceiptSetV1 {
        ProofReceiptSetV1::new(
            plan_id,
            vec![ProofReceiptBindingV1 {
                binding_id: format!("{plan_id}#0"),
                plan_id: plan_id.to_string(),
                command_index: 0,
                analysis_receipt_schema_id: "repo.analysis-receipt.v1".to_string(),
                receipt_digest: "sha256:v1:binding".to_string(),
            }],
        )
    }

    #[test]
    fn changed_intent_plan_gets_distinct_cache_identity() -> Result<(), String> {
        let base = sample_envelope();
        let mut changed = sample_envelope();
        if let Some(first) = changed.obligations.first_mut() {
            first.evidence_refs.push("doc:NEW.md".to_string());
        }
        let key_base = cache_key_for_intent_plan(&base).map_err(|err| err.as_str().to_string())?;
        let key_changed =
            cache_key_for_intent_plan(&changed).map_err(|err| err.as_str().to_string())?;
        if key_base == key_changed {
            return Err("changed intent plan must not share a cache identity".into());
        }
        Ok(())
    }

    #[test]
    fn receipts_captured_for_one_intent_plan_do_not_serve_another() -> Result<(), String> {
        let base = sample_envelope();
        let mut changed = sample_envelope();
        if let Some(first) = changed.obligations.first_mut() {
            first.statement = "Different statement".to_string();
        }

        let key_base = cache_key_for_intent_plan(&base).map_err(|err| err.as_str().to_string())?;
        let key_changed =
            cache_key_for_intent_plan(&changed).map_err(|err| err.as_str().to_string())?;

        let mut cache = ProofCacheV1::new();
        cache
            .insert(key_base.clone(), receipt_set("plan-a"))
            .map_err(|err| err.as_str().to_string())?;

        if cache.get(&key_base).is_none() {
            return Err("cache must serve the exact intent plan it captured".into());
        }
        if cache.get(&key_changed).is_some() {
            return Err("changed intent plan must not reuse captured receipts".into());
        }
        Ok(())
    }
}
