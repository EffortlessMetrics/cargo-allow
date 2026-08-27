//! Pure candidate-admission contracts for one-writer repository work (#3974).
//!
//! This module deliberately owns observations and decisions only. It performs
//! no repository, GitHub, filesystem, Cursor, or reservation operations.

use serde::{Deserialize, Serialize};

use crate::stable_hash_hex;

pub const CLAIM_REF_SCHEMA_V1: &str = "cargo-allow.claim-ref.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRefV1 {
    pub repository: String,
    pub controlling_issue: u64,
    pub change: String,
    pub semantic_route: String,
    pub claim: String,
    pub writer_key: String,
    pub accepted_base: String,
    pub claim_boundary: String,
}

impl ClaimRefV1 {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("repository", &self.repository),
            ("change", &self.change),
            ("semantic_route", &self.semantic_route),
            ("claim", &self.claim),
            ("writer_key", &self.writer_key),
            ("claim_boundary", &self.claim_boundary),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must be non-empty"));
            }
        }
        if self.controlling_issue == 0 {
            return Err("controlling_issue must be non-zero".into());
        }
        validate_sha(&self.accepted_base, "accepted_base")
    }

    pub fn identity(&self) -> Result<String, String> {
        self.validate()?;
        let canonical = [
            CLAIM_REF_SCHEMA_V1,
            &self.repository,
            &self.controlling_issue.to_string(),
            &self.change,
            &self.semantic_route,
            &self.claim,
            &self.writer_key,
            &self.accepted_base,
            &self.claim_boundary,
        ]
        .join("\u{1f}");
        Ok(stable_hash_hex(&canonical))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDispositionV1 {
    Reuse,
    Resume,
    Repair,
    Restack,
    Create,
    Wait,
    Reconcile,
    ReturnToDecision,
    Blocked,
    Stop,
    NotProven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStateV1 {
    Suitable,
    NeedsRepair,
    StaleBase,
    ActiveWriter,
    Satisfied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateObservationV1 {
    pub claim_identity: String,
    pub base: String,
    pub head: String,
    pub state: CandidateStateV1,
    pub active_writer: bool,
    pub repository_matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateObservationSetV1 {
    pub claim: ClaimRefV1,
    pub inventory_complete: bool,
    pub repository_current: bool,
    pub environment_capable: bool,
    pub semantic_premise_current: bool,
    pub candidates: Vec<CandidateObservationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateAdmissionDecisionV1 {
    pub disposition: CandidateDispositionV1,
    pub claim_identity: Option<String>,
    pub candidate_index: Option<usize>,
    pub reasons: Vec<String>,
}

impl CandidateObservationSetV1 {
    pub fn admit(&self) -> CandidateAdmissionDecisionV1 {
        let claim_identity = self.claim.identity().ok();
        let not_proven = |reason: &str| CandidateAdmissionDecisionV1 {
            disposition: CandidateDispositionV1::NotProven,
            claim_identity: claim_identity.clone(),
            candidate_index: None,
            reasons: vec![reason.into()],
        };
        if self.claim.validate().is_err() {
            return not_proven("claim identity is invalid");
        }
        if !self.inventory_complete {
            return not_proven("candidate inventory is incomplete");
        }
        if !self.repository_current {
            return not_proven("repository observation is not current");
        }
        if !self.semantic_premise_current {
            return CandidateAdmissionDecisionV1 {
                disposition: CandidateDispositionV1::ReturnToDecision,
                claim_identity,
                candidate_index: None,
                reasons: vec!["semantic premise is not current".into()],
            };
        }
        if !self.environment_capable {
            return CandidateAdmissionDecisionV1 {
                disposition: CandidateDispositionV1::Blocked,
                claim_identity,
                candidate_index: None,
                reasons: vec!["declared environment is not capable".into()],
            };
        }

        if self.candidates.iter().any(|candidate| {
            !candidate.repository_matches
                || validate_sha(&candidate.base, "candidate base").is_err()
                || validate_sha(&candidate.head, "candidate head").is_err()
        }) {
            return not_proven("candidate observation contains an invalid repository or object id");
        }

        let matching: Vec<_> = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.repository_matches
                    && candidate.claim_identity == claim_identity.as_deref().unwrap_or_default()
            })
            .collect();
        if matching.len() > 1 {
            return CandidateAdmissionDecisionV1 {
                disposition: CandidateDispositionV1::Reconcile,
                claim_identity,
                candidate_index: None,
                reasons: vec!["multiple candidates match the ClaimRef".into()],
            };
        }
        if let Some((index, candidate)) = matching.into_iter().next() {
            if candidate.active_writer {
                return CandidateAdmissionDecisionV1 {
                    disposition: CandidateDispositionV1::Wait,
                    claim_identity,
                    candidate_index: Some(index),
                    reasons: vec!["matching candidate has an active writer".into()],
                };
            }
            let disposition = match candidate.state {
                CandidateStateV1::Suitable => CandidateDispositionV1::Reuse,
                CandidateStateV1::NeedsRepair => CandidateDispositionV1::Repair,
                CandidateStateV1::StaleBase => CandidateDispositionV1::Restack,
                CandidateStateV1::Satisfied => CandidateDispositionV1::Stop,
                CandidateStateV1::ActiveWriter => CandidateDispositionV1::Wait,
            };
            return CandidateAdmissionDecisionV1 {
                disposition,
                claim_identity,
                candidate_index: Some(index),
                reasons: vec!["existing matching candidate selected".into()],
            };
        }
        CandidateAdmissionDecisionV1 {
            disposition: CandidateDispositionV1::Create,
            claim_identity,
            candidate_index: None,
            reasons: vec!["complete current observations permit reservation attempt".into()],
        }
    }
}

fn validate_sha(value: &str, name: &str) -> Result<(), String> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must be a full 40-character hexadecimal object id"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim() -> ClaimRefV1 {
        ClaimRefV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            controlling_issue: 3974,
            change: "candidate-admission".into(),
            semantic_route: "intent-model.agentic_candidate".into(),
            claim: "one-writer admission".into(),
            writer_key: "candidate-admission".into(),
            accepted_base: "0123456789abcdef0123456789abcdef01234567".into(),
            claim_boundary: "pure DTO and evaluator".into(),
        }
    }

    fn observations() -> CandidateObservationSetV1 {
        CandidateObservationSetV1 {
            claim: claim(),
            inventory_complete: true,
            repository_current: true,
            environment_capable: true,
            semantic_premise_current: true,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn complete_empty_inventory_admits_create() {
        assert_eq!(
            observations().admit().disposition,
            CandidateDispositionV1::Create
        );
    }

    #[test]
    fn matching_active_writer_waits() {
        let mut set = observations();
        let identity = set.claim.identity().unwrap();
        set.candidates.push(CandidateObservationV1 {
            claim_identity: identity,
            base: set.claim.accepted_base.clone(),
            head: "fedcba9876543210fedcba9876543210fedcba98".into(),
            state: CandidateStateV1::Suitable,
            active_writer: true,
            repository_matches: true,
        });
        assert_eq!(set.admit().disposition, CandidateDispositionV1::Wait);
    }

    #[test]
    fn duplicate_matching_candidates_reconcile() {
        let mut set = observations();
        let identity = set.claim.identity().unwrap();
        let candidate = CandidateObservationV1 {
            claim_identity: identity,
            base: set.claim.accepted_base.clone(),
            head: "fedcba9876543210fedcba9876543210fedcba98".into(),
            state: CandidateStateV1::Suitable,
            active_writer: false,
            repository_matches: true,
        };
        set.candidates = vec![candidate.clone(), candidate];
        assert_eq!(set.admit().disposition, CandidateDispositionV1::Reconcile);
    }

    #[test]
    fn incomplete_inventory_cannot_create() {
        let mut set = observations();
        set.inventory_complete = false;
        assert_eq!(set.admit().disposition, CandidateDispositionV1::NotProven);
    }

    #[test]
    fn malformed_base_cannot_create() {
        let mut set = observations();
        set.claim.accepted_base = "main".into();
        assert_eq!(set.admit().disposition, CandidateDispositionV1::NotProven);
    }

    #[test]
    fn identity_ignores_runtime_metadata() {
        let first = claim().identity().unwrap();
        let mut second = claim();
        second.claim_boundary = "same semantic boundary".into();
        assert_ne!(first, second.identity().unwrap());
    }
}
