//! Pure candidate-admission contracts for one-writer repository work (#3974).
//!
//! This module deliberately owns observations and decisions only. It performs
//! no repository, GitHub, filesystem, Cursor, or reservation operations.
//!
//! [`ClaimRefV1`] identity joins its free-form fields with the reserved
//! U+001F separator, so [`ClaimRefV1::validate`] rejects every C0 control
//! character and DEL in those fields (reusing the shared
//! [`reject_identity_control_characters`] rule). Without the rejection two
//! distinct claims — `change = "a\u{1f}b"` with `semantic_route = "c"` versus
//! `change = "a"` with `semantic_route = "b\u{1f}c"` — would both validate and
//! collapse into one identity.

use serde::{Deserialize, Serialize};

use crate::agentic_review_profile::reject_identity_control_characters;
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
            // The identity encoding joins these fields with U+001F, so every
            // control character (including both reserved separators) and DEL
            // is rejected to keep the claim identity injective.
            reject_identity_control_characters(name, value)?;
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
    Recoverable,
    NeedsRepair,
    StaleBase,
    ActiveWriter,
    Conflicting,
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
            if candidate.state == CandidateStateV1::Conflicting {
                return CandidateAdmissionDecisionV1 {
                    disposition: CandidateDispositionV1::Reconcile,
                    claim_identity,
                    candidate_index: Some(index),
                    reasons: vec!["matching candidate conflicts with the claim".into()],
                };
            }
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
                CandidateStateV1::Recoverable => CandidateDispositionV1::Resume,
                CandidateStateV1::NeedsRepair => CandidateDispositionV1::Repair,
                CandidateStateV1::StaleBase => CandidateDispositionV1::Restack,
                CandidateStateV1::Conflicting => CandidateDispositionV1::Reconcile,
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
    fn matching_active_writer_waits() -> Result<(), String> {
        let mut set = observations();
        let identity = set.claim.identity()?;
        set.candidates.push(CandidateObservationV1 {
            claim_identity: identity,
            base: set.claim.accepted_base.clone(),
            head: "fedcba9876543210fedcba9876543210fedcba98".into(),
            state: CandidateStateV1::Suitable,
            active_writer: true,
            repository_matches: true,
        });
        assert_eq!(set.admit().disposition, CandidateDispositionV1::Wait);
        Ok(())
    }

    #[test]
    fn recoverable_candidate_resumes() -> Result<(), String> {
        let mut set = observations();
        set.candidates.push(CandidateObservationV1 {
            claim_identity: set.claim.identity()?,
            base: set.claim.accepted_base.clone(),
            head: "fedcba9876543210fedcba9876543210fedcba98".into(),
            state: CandidateStateV1::Recoverable,
            active_writer: false,
            repository_matches: true,
        });
        assert_eq!(set.admit().disposition, CandidateDispositionV1::Resume);
        Ok(())
    }

    #[test]
    fn conflicting_candidate_reconciles() -> Result<(), String> {
        let mut set = observations();
        set.candidates.push(CandidateObservationV1 {
            claim_identity: set.claim.identity()?,
            base: set.claim.accepted_base.clone(),
            head: "fedcba9876543210fedcba9876543210fedcba98".into(),
            state: CandidateStateV1::Conflicting,
            active_writer: false,
            repository_matches: true,
        });
        assert_eq!(set.admit().disposition, CandidateDispositionV1::Reconcile);
        Ok(())
    }

    #[test]
    fn duplicate_matching_candidates_reconcile() -> Result<(), String> {
        let mut set = observations();
        let identity = set.claim.identity()?;
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
        Ok(())
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
    fn identity_ignores_runtime_metadata() -> Result<(), String> {
        let first = claim().identity()?;
        let mut second = claim();
        second.claim_boundary = "same semantic boundary".into();
        assert_ne!(first, second.identity()?);
        Ok(())
    }

    #[test]
    fn clean_claim_ref_still_validates() -> Result<(), String> {
        let clean = claim();
        clean.validate()?;
        assert!(clean.identity()?.starts_with("fnv1a64:"));
        Ok(())
    }

    #[test]
    fn nested_separator_collision_pair_in_claim_ref_is_rejected() -> Result<(), String> {
        // The #3976 PR B review collision pair: `ClaimRefV1::identity` joins
        // change/semantic_route with U+001F, so before the control-character
        // rejection these two claims both validated and shared one identity
        // (the canonical stream "a" U+001F "b" U+001F "c").
        let mut first = claim();
        first.change = "a\u{1f}b".into();
        first.semantic_route = "c".into();
        let mut second = claim();
        second.change = "a".into();
        second.semantic_route = "b\u{1f}c".into();
        let first_error = first
            .validate()
            .err()
            .ok_or("expected first claim-ref separator rejection")?;
        let second_error = second
            .validate()
            .err()
            .ok_or("expected second claim-ref separator rejection")?;
        assert!(
            first_error.contains("change") && first_error.contains("U+001F"),
            "the rejection must name the field and the reserved code point: {first_error}"
        );
        assert!(
            second_error.contains("C0 control characters"),
            "the rejection must name the character class: {second_error}"
        );
        assert!(first.identity().is_err());
        assert!(second.identity().is_err());
        Ok(())
    }

    #[test]
    fn list_separator_and_del_in_claim_ref_fields_are_rejected() -> Result<(), String> {
        let mut joined = claim();
        joined.claim_boundary = "boundary\u{1e}private".into();
        let joined_error = joined
            .validate()
            .err()
            .ok_or("expected claim_boundary U+001E rejection")?;
        assert!(
            joined_error.contains("claim_boundary") && joined_error.contains("U+001E"),
            "the rejection must name the field and the reserved code point: {joined_error}"
        );

        let mut del = claim();
        del.writer_key = "writer\u{7f}key".into();
        let del_error = del
            .validate()
            .err()
            .ok_or("expected writer_key DEL rejection")?;
        assert!(
            del_error.contains("writer_key") && del_error.contains("U+007F"),
            "the rejection must name the field and DEL: {del_error}"
        );

        let mut newline = claim();
        newline.claim = "claim\u{a}with newline".into();
        let newline_error = newline
            .validate()
            .err()
            .ok_or("expected claim newline rejection")?;
        assert!(
            newline_error.contains("claim must not contain C0 control characters"),
            "newlines are C0 control characters and must be rejected: {newline_error}"
        );
        Ok(())
    }
}
