//! Candidate-reservation contracts for global create exclusivity (#3975).
//!
//! This module owns the reservation request/observation/receipt DTOs, the
//! canonical remote candidate ref derivation, one deterministic
//! revalidate -> create-only -> read-back -> classify transition, and the
//! transport boundary. It performs no repository, filesystem, Cursor, or
//! network operations. [`InMemoryCandidateRefTransport`] is the deterministic
//! fixture; [`crate::agentic_reservation_gh`] adapts the same boundary to the
//! `gh` CLI.
//!
//! # Canonical ref derivation
//!
//! The canonical candidate ref for one [`ClaimRefV1`] is always
//! `refs/heads/cargo-allow/claims/<digest>` where `<digest>` is the
//! 16-hex-digit payload of the ClaimRef stable identity (`fnv1a64:<digest>`,
//! see [`crate::stable_hash_hex`]). The scheme prefix is dropped because git
//! ref names cannot contain `:`. The derivation is deterministic, total over
//! valid claims, and branch-compatible under `git-check-ref-format`.
//!
//! # Transition law (one call, exactly one result)
//!
//! 1. Structural request validation (admission is `Create`, claim identity
//!    binds the admission decision, exact base matches the claim, request and
//!    generation identifiers are present). Failure: `ValidationRejected`
//!    before any transport call.
//! 2. Immediate pre-mutation revalidation of the caller-supplied
//!    [`CandidateReservationObservationV1`] against the request: repository
//!    and remote identity, claim identity, exact accepted base,
//!    candidate-inventory generation and completeness, currentness,
//!    semantic premise, admission decision identity, and the selected #3983
//!    environment prerequisite. Movement: `ReservationStale` before any
//!    transport call.
//! 3. Create-only ref creation at the exact accepted base. Provider
//!    unavailability and rate-limit/abuse failures classify immediately
//!    (`ProviderUnavailable` / `RateLimitedOrAbuseProtected`) without a
//!    read-back, so a struggling provider is not hammered. Validation and
//!    unknown create failures never classify by response text alone: the
//!    read-back below is load-bearing.
//! 4. Exact read-back of the canonical ref and the candidate anchor, then
//!    classification into exactly one result:
//!    - ref present at the exact base with no candidate: `Reserved` (create
//!      succeeded) or `ExistingMatchingReservation` (create refused or
//!      already existed);
//!    - candidate anchored at the exact accepted base:
//!      `ExistingMatchingCandidate`;
//!    - ref or candidate present with an incompatible base:
//!      `ConflictingReservation`;
//!    - create refused and provider state is complete with no ref and no
//!      candidate: `ValidationRejected`;
//!    - create reported success (or an existing ref) but the read-back
//!      cannot observe the ref: `NotProven`. The deletion-between-create-
//!      and-read-back case is deliberately `NotProven`, not
//!      `InstrumentFailure`: the reservation postcondition is unproven and a
//!      retry is idempotent. `InstrumentFailure` is reserved for transport
//!      answers that cannot be decoded into typed facts.
//!
//! Only `Reserved` grants this caller ownership of the subsequent
//! candidate-creation transition; it never creates a worktree, PR, or
//! Cursor agent. Repeating one reservation request is idempotent: the second
//! attempt observes the first reservation and returns
//! `ExistingMatchingReservation`. This transition never deletes or retargets
//! an existing ref.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::agentic_candidate::{CandidateAdmissionDecisionV1, CandidateDispositionV1, ClaimRefV1};
use crate::error::stable_hash_hex;

/// Scheme prefix emitted by [`crate::stable_hash_hex`].
const IDENTITY_SCHEME: &str = "fnv1a64";

/// Canonical fully qualified remote namespace for candidate reservation refs.
pub const CANDIDATE_REF_NAMESPACE: &str = "refs/heads/cargo-allow/claims";

/// Validate one full 40-character hexadecimal git object id.
pub fn validate_object_id(value: &str, name: &str) -> Result<(), String> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must be a full 40-character hexadecimal object id"
        ));
    }
    Ok(())
}

/// Extract the branch-compatible digest payload of one ClaimRef identity.
///
/// The identity has the shape `fnv1a64:<16 hex>`; only the digest survives
/// into the ref because git ref names cannot contain `:`.
pub fn canonical_candidate_digest(claim_identity: &str) -> Result<String, String> {
    let mut segments = claim_identity.split(':');
    let scheme = segments.next().unwrap_or_default();
    let digest = segments.next().unwrap_or_default();
    if segments.next().is_some() {
        return Err("claim identity must be exactly scheme and digest".into());
    }
    if scheme != IDENTITY_SCHEME {
        return Err(format!("unsupported claim identity scheme: {scheme}"));
    }
    if digest.len() != 16
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("claim identity digest must be 16 lowercase hexadecimal characters".into());
    }
    Ok(digest.to_string())
}

/// Derive the canonical candidate ref from one ClaimRef identity.
pub fn canonical_candidate_ref_for_identity(claim_identity: &str) -> Result<String, String> {
    Ok(format!(
        "{CANDIDATE_REF_NAMESPACE}/{}",
        canonical_candidate_digest(claim_identity)?
    ))
}

/// Derive the canonical candidate ref from one ClaimRef.
pub fn canonical_candidate_ref(claim: &ClaimRefV1) -> Result<String, String> {
    canonical_candidate_ref_for_identity(&claim.identity()?)
}

/// Stable identity of one admission decision, used to detect admission
/// movement between decision and reservation.
pub fn admission_decision_identity(
    decision: &CandidateAdmissionDecisionV1,
) -> Result<String, String> {
    let canonical = serde_json::to_string(decision)
        .map_err(|error| format!("admission decision is not serializable: {error}"))?;
    Ok(stable_hash_hex(&canonical))
}

/// Selected #3983 environment capability prerequisite for one reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentCapabilityPrerequisiteV1 {
    pub capability_receipt_id: String,
    pub environment_generation: String,
}

/// One bounded reservation request: the current `Create` admission decision,
/// the current ClaimRef, the exact accepted base, and the optional #3983
/// environment prerequisite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateReservationRequestV1 {
    pub claim: ClaimRefV1,
    pub admission: CandidateAdmissionDecisionV1,
    pub accepted_base: String,
    pub inventory_generation: String,
    pub adapter_generation: String,
    pub request_id: String,
    pub environment_prerequisite: Option<EnvironmentCapabilityPrerequisiteV1>,
}

/// Current-world facts gathered for the immediate pre-mutation revalidation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateReservationObservationV1 {
    pub claim_identity: String,
    pub repository: String,
    pub accepted_base: String,
    pub inventory_complete: bool,
    pub inventory_generation: String,
    pub repository_current: bool,
    pub semantic_premise_current: bool,
    pub environment_capable: bool,
    pub environment_prerequisite_id: Option<String>,
    pub admission_decision_id: String,
}

/// Closed reservation result vocabulary. Only `Reserved` grants this caller
/// ownership of the subsequent candidate-creation transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateReservationResultV1 {
    Reserved,
    ExistingMatchingReservation,
    ExistingMatchingCandidate,
    ConflictingReservation,
    ReservationStale,
    ValidationRejected,
    ProviderUnavailable,
    RateLimitedOrAbuseProtected,
    InstrumentFailure,
    NotProven,
}

/// Receipt binding the reservation identity and outcome. `complete` is true
/// exactly when the classification rests on a complete, classification-grade
/// view of the world (pre-transport rejections and full read-backs); it is
/// false for unavailable, rate-limited, undecodable, and unproven postures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateReservationReceiptV1 {
    pub result: CandidateReservationResultV1,
    pub complete: bool,
    pub reasons: Vec<String>,
    pub claim_identity: Option<String>,
    pub repository: String,
    pub canonical_ref: String,
    pub accepted_base: String,
    pub request_id: String,
    pub adapter_generation: String,
    pub inventory_generation: String,
    pub environment_prerequisite: Option<EnvironmentCapabilityPrerequisiteV1>,
    pub claim_boundary: String,
}

/// One create-ref command over the transport boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRefCommandV1 {
    pub repository: String,
    pub reference: String,
    pub target_sha: String,
}

/// Create-only outcome. `AlreadyExists` is evidence, never classification:
/// the caller must read back before classifying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateRefOutcomeV1 {
    Created,
    AlreadyExists,
}

/// Read-back facts for one remote ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefReadBackV1 {
    pub target_sha: String,
}

/// Read-back facts for the candidate anchor (for example an open pull
/// request) attached to one remote ref.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CandidateAnchorReadBackV1 {
    pub candidate_exists: bool,
    pub candidate_base: String,
}

/// Provider-failure vocabulary shared by every transport operation. Response
/// text alone never grants ownership; these failures only bound what the
/// transition may classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportFailureV1 {
    ProviderUnavailable(String),
    RateLimitedOrAbuseProtected(String),
    ValidationRejected(String),
    InstrumentFailure(String),
}

/// Transport boundary for the reservation transition: create-ref and
/// read-ref operations returning provider-failure-mapped errors.
pub trait CandidateRefTransport {
    fn create_ref(
        &mut self,
        command: &CreateRefCommandV1,
    ) -> Result<CreateRefOutcomeV1, TransportFailureV1>;

    fn read_ref(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<Option<RefReadBackV1>, TransportFailureV1>;

    fn read_candidate_anchor(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<CandidateAnchorReadBackV1, TransportFailureV1>;
}

/// Scripted responses for the fixture create-ref operation. `Created`
/// performs the mutation; `AlreadyExists` performs no mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureCreateResponse {
    Created,
    AlreadyExists,
    Refuse(TransportFailureV1),
}

/// Scripted responses for the fixture read-ref operation. `State` reflects
/// the fixture store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureReadRefResponse {
    State,
    Absent,
    Present(String),
    Refuse(TransportFailureV1),
}

/// Scripted responses for the fixture candidate-anchor operation. `State`
/// reflects the fixture store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureCandidateResponse {
    State,
    Absent,
    Present { base: String },
    Refuse(TransportFailureV1),
}

/// Deterministic in-memory [`CandidateRefTransport`]. Without a queued
/// response each operation reflects the fixture store, so first-create wins
/// and later creates report `AlreadyExists`. Queued responses are consumed
/// first-in-first-out per operation; there are no threads and no timing.
#[derive(Debug, Clone)]
pub struct InMemoryCandidateRefTransport {
    repository: String,
    refs: BTreeMap<String, String>,
    candidates: BTreeMap<String, String>,
    create_script: VecDeque<FixtureCreateResponse>,
    read_ref_script: VecDeque<FixtureReadRefResponse>,
    read_candidate_script: VecDeque<FixtureCandidateResponse>,
    calls: Vec<String>,
}

impl InMemoryCandidateRefTransport {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            refs: BTreeMap::new(),
            candidates: BTreeMap::new(),
            create_script: VecDeque::new(),
            read_ref_script: VecDeque::new(),
            read_candidate_script: VecDeque::new(),
            calls: Vec::new(),
        }
    }

    pub fn seed_reservation(&mut self, reference: &str, target_sha: &str) {
        self.refs
            .insert(reference.to_string(), target_sha.to_string());
    }

    pub fn seed_candidate(&mut self, reference: &str, base_sha: &str) {
        self.candidates
            .insert(reference.to_string(), base_sha.to_string());
    }

    pub fn queue_create(&mut self, response: FixtureCreateResponse) {
        self.create_script.push_back(response);
    }

    pub fn queue_read_ref(&mut self, response: FixtureReadRefResponse) {
        self.read_ref_script.push_back(response);
    }

    pub fn queue_read_candidate(&mut self, response: FixtureCandidateResponse) {
        self.read_candidate_script.push_back(response);
    }

    pub fn transport_calls(&self) -> &[String] {
        &self.calls
    }

    pub fn ref_target(&self, reference: &str) -> Option<&str> {
        self.refs.get(reference).map(String::as_str)
    }

    fn check_repository(&self, repository: &str) -> Result<(), TransportFailureV1> {
        if repository == self.repository {
            return Ok(());
        }
        Err(TransportFailureV1::ValidationRejected(format!(
            "fixture transport is bound to repository {} and cannot serve {repository}",
            self.repository
        )))
    }
}

impl CandidateRefTransport for InMemoryCandidateRefTransport {
    fn create_ref(
        &mut self,
        command: &CreateRefCommandV1,
    ) -> Result<CreateRefOutcomeV1, TransportFailureV1> {
        self.calls.push("create_ref".into());
        self.check_repository(&command.repository)?;
        match self.create_script.pop_front() {
            Some(FixtureCreateResponse::Created) => {
                self.refs
                    .insert(command.reference.clone(), command.target_sha.clone());
                Ok(CreateRefOutcomeV1::Created)
            }
            Some(FixtureCreateResponse::AlreadyExists) => Ok(CreateRefOutcomeV1::AlreadyExists),
            Some(FixtureCreateResponse::Refuse(failure)) => Err(failure),
            None => {
                if self.refs.contains_key(&command.reference) {
                    return Ok(CreateRefOutcomeV1::AlreadyExists);
                }
                self.refs
                    .insert(command.reference.clone(), command.target_sha.clone());
                Ok(CreateRefOutcomeV1::Created)
            }
        }
    }

    fn read_ref(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<Option<RefReadBackV1>, TransportFailureV1> {
        self.calls.push("read_ref".into());
        self.check_repository(repository)?;
        match self.read_ref_script.pop_front() {
            Some(FixtureReadRefResponse::Absent) => Ok(None),
            Some(FixtureReadRefResponse::Present(target_sha)) => {
                Ok(Some(RefReadBackV1 { target_sha }))
            }
            Some(FixtureReadRefResponse::Refuse(failure)) => Err(failure),
            Some(FixtureReadRefResponse::State) | None => {
                Ok(self.refs.get(reference).map(|sha| RefReadBackV1 {
                    target_sha: sha.clone(),
                }))
            }
        }
    }

    fn read_candidate_anchor(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<CandidateAnchorReadBackV1, TransportFailureV1> {
        self.calls.push("read_candidate_anchor".into());
        self.check_repository(repository)?;
        match self.read_candidate_script.pop_front() {
            Some(FixtureCandidateResponse::Absent) => Ok(CandidateAnchorReadBackV1::default()),
            Some(FixtureCandidateResponse::Present { base }) => Ok(CandidateAnchorReadBackV1 {
                candidate_exists: true,
                candidate_base: base,
            }),
            Some(FixtureCandidateResponse::Refuse(failure)) => Err(failure),
            Some(FixtureCandidateResponse::State) | None => {
                Ok(self.candidates.get(reference).map_or_else(
                    CandidateAnchorReadBackV1::default,
                    |base| CandidateAnchorReadBackV1 {
                        candidate_exists: true,
                        candidate_base: base.clone(),
                    },
                ))
            }
        }
    }
}

fn receipt_skeleton(
    request: &CandidateReservationRequestV1,
    claim_identity: Option<String>,
    canonical_ref: String,
    result: CandidateReservationResultV1,
    complete: bool,
    reasons: Vec<String>,
) -> CandidateReservationReceiptV1 {
    CandidateReservationReceiptV1 {
        result,
        complete,
        reasons,
        claim_identity,
        repository: request.claim.repository.clone(),
        canonical_ref,
        accepted_base: request.accepted_base.clone(),
        request_id: request.request_id.clone(),
        adapter_generation: request.adapter_generation.clone(),
        inventory_generation: request.inventory_generation.clone(),
        environment_prerequisite: request.environment_prerequisite.clone(),
        claim_boundary: request.claim.claim_boundary.clone(),
    }
}

/// Run one reservation transition against one transport and classify the
/// outcome into exactly one result. The transition is total: every failure
/// mode folds into the receipt rather than surfacing as an error.
pub fn reserve_candidate_ref(
    transport: &mut dyn CandidateRefTransport,
    request: &CandidateReservationRequestV1,
    observation: &CandidateReservationObservationV1,
) -> CandidateReservationReceiptV1 {
    // Phase 1: structural request validation, before any transport call.
    let identity = match request.claim.identity() {
        Ok(value) => value,
        Err(reason) => {
            return receipt_skeleton(
                request,
                None,
                String::new(),
                CandidateReservationResultV1::ValidationRejected,
                true,
                vec![format!("claim identity is invalid: {reason}")],
            );
        }
    };
    let canonical_ref = match canonical_candidate_ref_for_identity(&identity) {
        Ok(value) => value,
        Err(reason) => {
            return receipt_skeleton(
                request,
                Some(identity),
                String::new(),
                CandidateReservationResultV1::ValidationRejected,
                true,
                vec![format!("canonical ref derivation failed: {reason}")],
            );
        }
    };
    let reject = |reason: String| {
        receipt_skeleton(
            request,
            Some(identity.clone()),
            canonical_ref.clone(),
            CandidateReservationResultV1::ValidationRejected,
            true,
            vec![reason],
        )
    };
    if let Err(reason) = validate_object_id(&request.accepted_base, "accepted_base") {
        return reject(reason);
    }
    if request.accepted_base != request.claim.accepted_base {
        return reject("accepted_base must match the ClaimRef accepted base".into());
    }
    for (name, value) in [
        ("request_id", &request.request_id),
        ("adapter_generation", &request.adapter_generation),
        ("inventory_generation", &request.inventory_generation),
    ] {
        if value.trim().is_empty() {
            return reject(format!("{name} must be non-empty"));
        }
    }
    if request.admission.disposition != CandidateDispositionV1::Create {
        return reject("reservation requires a current Create admission decision".into());
    }
    if request.admission.claim_identity.as_deref() != Some(identity.as_str()) {
        return reject("admission decision does not bind this ClaimRef identity".into());
    }
    let admission_id = match admission_decision_identity(&request.admission) {
        Ok(value) => value,
        Err(reason) => return reject(reason),
    };
    if let Some(prerequisite) = &request.environment_prerequisite
        && (prerequisite.capability_receipt_id.trim().is_empty()
            || prerequisite.environment_generation.trim().is_empty())
    {
        return reject("environment prerequisite identity is incomplete".into());
    }

    // Phase 2: immediate pre-mutation revalidation of the current world,
    // still before any transport call.
    let stale = |reason: String| {
        receipt_skeleton(
            request,
            Some(identity.clone()),
            canonical_ref.clone(),
            CandidateReservationResultV1::ReservationStale,
            true,
            vec![reason],
        )
    };
    if observation.repository != request.claim.repository {
        return reject("observation belongs to another repository or remote".into());
    }
    if observation.claim_identity != identity {
        return stale("observation no longer matches the ClaimRef identity".into());
    }
    if observation.accepted_base != request.accepted_base {
        return stale("exact accepted base moved after admission".into());
    }
    if observation.inventory_generation != request.inventory_generation {
        return stale("candidate inventory generation moved after admission".into());
    }
    if observation.admission_decision_id != admission_id {
        return stale("admission decision identity moved after admission".into());
    }
    if !observation.repository_current {
        return stale("repository observation is not current".into());
    }
    if !observation.inventory_complete {
        return stale("candidate inventory is incomplete".into());
    }
    if !observation.semantic_premise_current {
        return stale("semantic premise is not current".into());
    }
    match (
        &request.environment_prerequisite,
        &observation.environment_prerequisite_id,
    ) {
        (Some(prerequisite), Some(observed_id))
            if observed_id == &prerequisite.capability_receipt_id =>
        {
            if !observation.environment_capable {
                return stale("#3983 environment is not capable for the selected receipt".into());
            }
        }
        (Some(_), _) => {
            return stale("#3983 environment prerequisite is not current".into());
        }
        (None, _) => {}
    }

    // Phase 3: create-only mutation at the exact accepted base. Provider
    // unavailability and rate limiting classify immediately without a
    // read-back; validation and unknown failures fall through to the
    // load-bearing read-back.
    enum CreatePhase {
        Created,
        AlreadyExists,
        Refused(String),
    }
    let phase = match transport.create_ref(&CreateRefCommandV1 {
        repository: request.claim.repository.clone(),
        reference: canonical_ref.clone(),
        target_sha: request.accepted_base.clone(),
    }) {
        Ok(CreateRefOutcomeV1::Created) => CreatePhase::Created,
        Ok(CreateRefOutcomeV1::AlreadyExists) => CreatePhase::AlreadyExists,
        Err(TransportFailureV1::ValidationRejected(detail))
        | Err(TransportFailureV1::InstrumentFailure(detail)) => CreatePhase::Refused(detail),
        Err(TransportFailureV1::ProviderUnavailable(detail)) => {
            return receipt_skeleton(
                request,
                Some(identity),
                canonical_ref,
                CandidateReservationResultV1::ProviderUnavailable,
                false,
                vec![format!("provider is unavailable: {detail}")],
            );
        }
        Err(TransportFailureV1::RateLimitedOrAbuseProtected(detail)) => {
            return receipt_skeleton(
                request,
                Some(identity),
                canonical_ref,
                CandidateReservationResultV1::RateLimitedOrAbuseProtected,
                false,
                vec![format!(
                    "provider is rate limited or abuse protected: {detail}"
                )],
            );
        }
    };

    // Phase 4: exact read-back and classification.
    let read_failure_receipt = |request: &CandidateReservationRequestV1,
                                identity: &str,
                                canonical_ref: &str,
                                failure: TransportFailureV1| {
        let (result, complete, reason) = match failure {
            TransportFailureV1::ProviderUnavailable(detail) => (
                CandidateReservationResultV1::ProviderUnavailable,
                false,
                format!("provider is unavailable during read-back: {detail}"),
            ),
            TransportFailureV1::RateLimitedOrAbuseProtected(detail) => (
                CandidateReservationResultV1::RateLimitedOrAbuseProtected,
                false,
                format!("provider is rate limited or abuse protected during read-back: {detail}"),
            ),
            TransportFailureV1::ValidationRejected(detail)
            | TransportFailureV1::InstrumentFailure(detail) => (
                CandidateReservationResultV1::InstrumentFailure,
                false,
                format!("read-back could not observe the canonical ref: {detail}"),
            ),
        };
        receipt_skeleton(
            request,
            Some(identity.to_string()),
            canonical_ref.to_string(),
            result,
            complete,
            vec![reason],
        )
    };
    let existing = match transport.read_ref(&request.claim.repository, &canonical_ref) {
        Ok(found) => found,
        Err(failure) => {
            return read_failure_receipt(request, &identity, &canonical_ref, failure);
        }
    };
    let Some(ref_read) = existing else {
        let reason = match phase {
            CreatePhase::Created => {
                "created ref was absent at read-back; reservation postcondition is unproven"
                    .to_string()
            }
            CreatePhase::AlreadyExists => {
                "provider reported an existing ref that read-back cannot observe".to_string()
            }
            CreatePhase::Refused(ref detail) => {
                let anchor = match transport
                    .read_candidate_anchor(&request.claim.repository, &canonical_ref)
                {
                    Ok(anchor) => anchor,
                    Err(failure) => {
                        return read_failure_receipt(request, &identity, &canonical_ref, failure);
                    }
                };
                if anchor.candidate_exists && anchor.candidate_base == request.accepted_base {
                    return receipt_skeleton(
                        request,
                        Some(identity),
                        canonical_ref,
                        CandidateReservationResultV1::ExistingMatchingCandidate,
                        true,
                        vec![
                            "create was refused and an orphan candidate remains anchored at the exact base"
                                .into(),
                        ],
                    );
                }
                if anchor.candidate_exists {
                    return receipt_skeleton(
                        request,
                        Some(identity),
                        canonical_ref,
                        CandidateReservationResultV1::ConflictingReservation,
                        true,
                        vec![
                            "create was refused and a candidate with an incompatible base exists"
                                .into(),
                        ],
                    );
                }
                format!(
                    "provider refused create and complete read-back observes no ref or candidate: {detail}"
                )
            }
        };
        let (result, complete) = if matches!(phase, CreatePhase::Refused(_)) {
            (CandidateReservationResultV1::ValidationRejected, true)
        } else {
            (CandidateReservationResultV1::NotProven, false)
        };
        return receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            result,
            complete,
            vec![reason],
        );
    };
    let anchor = match transport.read_candidate_anchor(&request.claim.repository, &canonical_ref) {
        Ok(anchor) => anchor,
        Err(failure) => {
            return read_failure_receipt(request, &identity, &canonical_ref, failure);
        }
    };
    if ref_read.target_sha != request.accepted_base {
        let reason = if anchor.candidate_exists && anchor.candidate_base == request.accepted_base {
            "canonical ref advanced to a candidate anchored at the exact base".to_string()
        } else {
            "canonical ref exists at an incompatible base with no matching candidate".to_string()
        };
        let result = if anchor.candidate_exists && anchor.candidate_base == request.accepted_base {
            CandidateReservationResultV1::ExistingMatchingCandidate
        } else {
            CandidateReservationResultV1::ConflictingReservation
        };
        return receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            result,
            true,
            vec![reason],
        );
    }
    if anchor.candidate_exists && anchor.candidate_base != request.accepted_base {
        return receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            CandidateReservationResultV1::ConflictingReservation,
            true,
            vec!["ref exists at the exact base but the anchored candidate conflicts".into()],
        );
    }
    if anchor.candidate_exists {
        return receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            CandidateReservationResultV1::ExistingMatchingCandidate,
            true,
            vec!["candidate already anchored at the exact base".into()],
        );
    }
    match phase {
        CreatePhase::Created => receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            CandidateReservationResultV1::Reserved,
            true,
            vec!["created the canonical ref at the exact accepted base".into()],
        ),
        _ => receipt_skeleton(
            request,
            Some(identity),
            canonical_ref,
            CandidateReservationResultV1::ExistingMatchingReservation,
            true,
            vec!["compatible reservation already exists at the exact accepted base".into()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic_candidate::CandidateObservationSetV1;

    const OTHER_BASE: &str = "fedcba9876543210fedcba9876543210fedcba98";

    fn claim() -> ClaimRefV1 {
        ClaimRefV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            controlling_issue: 3975,
            change: "candidate-reservation".into(),
            semantic_route: "intent-model.agentic_reservation".into(),
            claim: "canonical candidate ref reservation".into(),
            writer_key: "candidate-reservation".into(),
            accepted_base: "0123456789abcdef0123456789abcdef01234567".into(),
            claim_boundary: "create-only exclusion token".into(),
        }
    }

    fn create_request() -> Result<CandidateReservationRequestV1, String> {
        let claim = claim();
        let admission = CandidateObservationSetV1 {
            claim: claim.clone(),
            inventory_complete: true,
            repository_current: true,
            environment_capable: true,
            semantic_premise_current: true,
            candidates: Vec::new(),
        }
        .admit();
        if admission.disposition != CandidateDispositionV1::Create {
            return Err("fixture admission must be Create".into());
        }
        Ok(CandidateReservationRequestV1 {
            accepted_base: claim.accepted_base.clone(),
            inventory_generation: "inventory-gen-1".into(),
            adapter_generation: "reservation-adapter-v1".into(),
            request_id: "req-3975-1".into(),
            environment_prerequisite: None,
            claim,
            admission,
        })
    }

    fn observation(
        request: &CandidateReservationRequestV1,
    ) -> Result<CandidateReservationObservationV1, String> {
        Ok(CandidateReservationObservationV1 {
            claim_identity: request.claim.identity()?,
            repository: request.claim.repository.clone(),
            accepted_base: request.accepted_base.clone(),
            inventory_complete: true,
            inventory_generation: request.inventory_generation.clone(),
            repository_current: true,
            semantic_premise_current: true,
            environment_capable: true,
            environment_prerequisite_id: request
                .environment_prerequisite
                .as_ref()
                .map(|item| item.capability_receipt_id.clone()),
            admission_decision_id: admission_decision_identity(&request.admission)?,
        })
    }

    fn canonical(request: &CandidateReservationRequestV1) -> Result<String, String> {
        canonical_candidate_ref(&request.claim)
    }

    #[test]
    fn canonical_ref_is_deterministic_and_branch_compatible() -> Result<(), String> {
        let first = canonical_candidate_ref(&claim())?;
        let second = canonical_candidate_ref(&claim())?;
        assert_eq!(first, second);
        let digest = canonical_candidate_digest(&claim().identity()?)?;
        let expected = format!("{CANDIDATE_REF_NAMESPACE}/{digest}");
        assert_eq!(first, expected);
        assert!(first.starts_with("refs/heads/cargo-allow/claims/"));
        assert!(!first.contains(':'));
        assert!(
            first.chars().all(|char| char.is_ascii_alphanumeric()
                || char == '/'
                || char == '.'
                || char == '-')
        );
        let mut other = claim();
        other.controlling_issue = 1;
        assert_ne!(first, canonical_candidate_ref(&other)?);
        Ok(())
    }

    #[test]
    fn digest_rejects_abbreviated_or_foreign_identities() {
        assert!(canonical_candidate_digest("").is_err());
        assert!(canonical_candidate_digest("0123456789abcdef").is_err());
        assert!(canonical_candidate_digest("fnv1a64:0123456789ABCDEF").is_err());
        assert!(canonical_candidate_digest("fnv1a64:0123456789abcdef:extra").is_err());
        assert_eq!(
            canonical_candidate_digest("fnv1a64:0123456789abcdef").ok(),
            Some("0123456789abcdef".into())
        );
    }

    #[test]
    fn first_request_reserves_canonical_ref_at_exact_base() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(receipt.result, CandidateReservationResultV1::Reserved);
        assert!(receipt.complete);
        assert_eq!(receipt.claim_identity, Some(request.claim.identity()?));
        assert_eq!(receipt.canonical_ref, reference);
        assert_eq!(receipt.accepted_base, request.accepted_base);
        assert_eq!(receipt.request_id, request.request_id);
        assert_eq!(receipt.adapter_generation, request.adapter_generation);
        assert_eq!(receipt.inventory_generation, request.inventory_generation);
        assert_eq!(receipt.claim_boundary, request.claim.claim_boundary);
        assert!(receipt.environment_prerequisite.is_none());
        assert_eq!(
            fixture.ref_target(&reference).map(str::to_string),
            Some(request.accepted_base.clone())
        );
        assert_eq!(
            fixture.transport_calls(),
            &[
                "create_ref".to_string(),
                "read_ref".to_string(),
                "read_candidate_anchor".to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn two_racing_controllers_yield_one_reserved_one_existing() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::Created);
        fixture.queue_create(FixtureCreateResponse::AlreadyExists);
        let first = reserve_candidate_ref(&mut fixture, &request, &observation);
        let second = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(first.result, CandidateReservationResultV1::Reserved);
        assert_eq!(
            second.result,
            CandidateReservationResultV1::ExistingMatchingReservation
        );
        assert!(second.complete);
        Ok(())
    }

    #[test]
    fn repeat_request_is_idempotent() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let first = reserve_candidate_ref(&mut fixture, &request, &observation);
        let second = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(first.result, CandidateReservationResultV1::Reserved);
        assert_eq!(
            second.result,
            CandidateReservationResultV1::ExistingMatchingReservation
        );
        assert_eq!(
            fixture
                .ref_target(&canonical(&request)?)
                .map(str::to_string),
            Some(request.accepted_base.clone())
        );
        Ok(())
    }

    #[test]
    fn existing_reservation_without_candidate_matches() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, &request.accepted_base);
        fixture.queue_create(FixtureCreateResponse::AlreadyExists);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ExistingMatchingReservation
        );
        assert!(receipt.complete);
        Ok(())
    }

    #[test]
    fn existing_matching_candidate_is_reused_not_replaced() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, OTHER_BASE);
        fixture.seed_candidate(&reference, &request.accepted_base);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ExistingMatchingCandidate
        );
        assert!(receipt.complete);
        Ok(())
    }

    #[test]
    fn candidate_anchored_on_reserved_ref_matches() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, &request.accepted_base);
        fixture.seed_candidate(&reference, &request.accepted_base);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ExistingMatchingCandidate
        );
        Ok(())
    }

    #[test]
    fn conflicting_ref_with_no_candidate_conflicts() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, OTHER_BASE);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ConflictingReservation
        );
        assert!(receipt.complete);
        Ok(())
    }

    #[test]
    fn candidate_with_incompatible_base_conflicts() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, OTHER_BASE);
        fixture.seed_candidate(&reference, OTHER_BASE);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ConflictingReservation
        );
        Ok(())
    }

    #[test]
    fn moved_base_after_admission_is_stale_before_transport() -> Result<(), String> {
        let request = create_request()?;
        let mut observation = observation(&request)?;
        observation.accepted_base = OTHER_BASE.into();
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(receipt.complete);
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn moved_inventory_generation_is_stale_before_transport() -> Result<(), String> {
        let request = create_request()?;
        let mut observation = observation(&request)?;
        observation.inventory_generation = "inventory-gen-2".into();
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn stale_admission_decision_is_stale_before_transport() -> Result<(), String> {
        let request = create_request()?;
        let mut observation = observation(&request)?;
        observation.admission_decision_id = "fnv1a64:ffffffffffffffff".into();
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn stale_environment_prerequisite_is_stale_before_transport() -> Result<(), String> {
        let mut request = create_request()?;
        request.environment_prerequisite = Some(EnvironmentCapabilityPrerequisiteV1 {
            capability_receipt_id: "cursor-env-3983".into(),
            environment_generation: "gen-1".into(),
        });
        let mut observation = observation(&request)?;
        observation.environment_prerequisite_id = None;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn incapable_environment_is_stale_before_transport() -> Result<(), String> {
        let mut request = create_request()?;
        request.environment_prerequisite = Some(EnvironmentCapabilityPrerequisiteV1 {
            capability_receipt_id: "cursor-env-3983".into(),
            environment_generation: "gen-1".into(),
        });
        let mut observation = observation(&request)?;
        observation.environment_capable = false;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn current_environment_prerequisite_reserves() -> Result<(), String> {
        let mut request = create_request()?;
        request.environment_prerequisite = Some(EnvironmentCapabilityPrerequisiteV1 {
            capability_receipt_id: "cursor-env-3983".into(),
            environment_generation: "gen-1".into(),
        });
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(receipt.result, CandidateReservationResultV1::Reserved);
        assert_eq!(
            receipt.environment_prerequisite,
            request.environment_prerequisite
        );
        Ok(())
    }

    #[test]
    fn another_repository_cannot_satisfy_the_request() -> Result<(), String> {
        let request = create_request()?;
        let mut observation = observation(&request)?;
        observation.repository = "EffortlessMetrics/other-repo".into();
        let mut fixture = InMemoryCandidateRefTransport::new("EffortlessMetrics/other-repo");
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ValidationRejected
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn malformed_base_is_rejected_before_transport() -> Result<(), String> {
        let mut request = create_request()?;
        request.accepted_base = "deadbee".into();
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ValidationRejected
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn non_create_admission_is_rejected_before_transport() -> Result<(), String> {
        let mut request = create_request()?;
        request.admission.disposition = CandidateDispositionV1::Reuse;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ValidationRejected
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn admission_bound_to_another_claim_is_rejected() -> Result<(), String> {
        let mut request = create_request()?;
        request.admission.claim_identity = Some("fnv1a64:ffffffffffffffff".into());
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ValidationRejected
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn ref_deleted_between_create_and_read_back_is_not_proven() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::Created);
        fixture.queue_read_ref(FixtureReadRefResponse::Absent);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(receipt.result, CandidateReservationResultV1::NotProven);
        assert!(!receipt.complete);
        Ok(())
    }

    #[test]
    fn contradictory_already_exists_without_ref_is_not_proven() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::AlreadyExists);
        fixture.queue_read_ref(FixtureReadRefResponse::Absent);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(receipt.result, CandidateReservationResultV1::NotProven);
        assert!(!receipt.complete);
        Ok(())
    }

    #[test]
    fn validation_failure_with_no_ref_and_no_candidate_rejects() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::Refuse(
            TransportFailureV1::ValidationRejected("gh: Validation Failed (http 422)".into()),
        ));
        fixture.queue_read_ref(FixtureReadRefResponse::Absent);
        fixture.queue_read_candidate(FixtureCandidateResponse::Absent);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ValidationRejected
        );
        assert!(receipt.complete);
        Ok(())
    }

    #[test]
    fn validation_failure_with_existing_ref_matches_reservation() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, &request.accepted_base);
        fixture.queue_create(FixtureCreateResponse::Refuse(
            TransportFailureV1::ValidationRejected("gh: Validation Failed (http 422)".into()),
        ));
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ExistingMatchingReservation
        );
        Ok(())
    }

    #[test]
    fn orphan_candidate_without_ref_remains_recoverable() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_candidate(&reference, &request.accepted_base);
        fixture.queue_create(FixtureCreateResponse::Refuse(
            TransportFailureV1::ValidationRejected("gh: Validation Failed (http 422)".into()),
        ));
        fixture.queue_read_ref(FixtureReadRefResponse::Absent);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ExistingMatchingCandidate
        );
        assert!(receipt.complete);
        Ok(())
    }

    #[test]
    fn rate_limited_create_classifies_without_read_back() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::Refuse(
            TransportFailureV1::RateLimitedOrAbuseProtected(
                "gh: API rate limit exceeded (http 403)".into(),
            ),
        ));
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::RateLimitedOrAbuseProtected
        );
        assert!(!receipt.complete);
        assert_eq!(fixture.transport_calls(), &["create_ref".to_string()]);
        Ok(())
    }

    #[test]
    fn unavailable_provider_classifies_without_read_back() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_create(FixtureCreateResponse::Refuse(
            TransportFailureV1::ProviderUnavailable("connection refused".into()),
        ));
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ProviderUnavailable
        );
        assert!(!receipt.complete);
        assert_eq!(fixture.transport_calls(), &["create_ref".to_string()]);
        Ok(())
    }

    #[test]
    fn undecodable_read_back_is_instrument_failure() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.queue_read_ref(FixtureReadRefResponse::Refuse(
            TransportFailureV1::InstrumentFailure("undecodable ref payload".into()),
        ));
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::InstrumentFailure
        );
        assert!(!receipt.complete);
        Ok(())
    }

    #[test]
    fn incomplete_currentness_is_stale_before_transport() -> Result<(), String> {
        let request = create_request()?;
        let mut observation = observation(&request)?;
        observation.repository_current = false;
        observation.inventory_complete = false;
        observation.semantic_premise_current = false;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ReservationStale
        );
        assert!(fixture.transport_calls().is_empty());
        Ok(())
    }

    #[test]
    fn malformed_observed_candidate_is_flagged_through_conflict_path() -> Result<(), String> {
        let request = create_request()?;
        let observation = observation(&request)?;
        let reference = canonical(&request)?;
        let mut fixture = InMemoryCandidateRefTransport::new(&request.claim.repository);
        fixture.seed_reservation(&reference, OTHER_BASE);
        let receipt = reserve_candidate_ref(&mut fixture, &request, &observation);
        assert_eq!(
            receipt.result,
            CandidateReservationResultV1::ConflictingReservation
        );
        Ok(())
    }

    #[test]
    fn admission_identity_is_stable_and_binding() -> Result<(), String> {
        let request = create_request()?;
        let identity = admission_decision_identity(&request.admission)?;
        let mut altered = request.admission.clone();
        altered.reasons.push("drift".into());
        assert_eq!(identity, admission_decision_identity(&request.admission)?);
        assert_ne!(identity, admission_decision_identity(&altered)?);
        Ok(())
    }

    #[test]
    fn fixture_default_create_is_create_only() -> Result<(), String> {
        let mut fixture = InMemoryCandidateRefTransport::new("EffortlessMetrics/cargo-allow");
        let command = CreateRefCommandV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            reference: "refs/heads/cargo-allow/claims/0123456789abcdef".into(),
            target_sha: claim().accepted_base,
        };
        assert_eq!(
            fixture.create_ref(&command).ok(),
            Some(CreateRefOutcomeV1::Created)
        );
        assert_eq!(
            fixture.create_ref(&command).ok(),
            Some(CreateRefOutcomeV1::AlreadyExists)
        );
        Ok(())
    }

    #[test]
    fn fixture_rejects_foreign_repository() {
        let mut fixture = InMemoryCandidateRefTransport::new("EffortlessMetrics/cargo-allow");
        let command = CreateRefCommandV1 {
            repository: "EffortlessMetrics/other-repo".into(),
            reference: "refs/heads/cargo-allow/claims/0123456789abcdef".into(),
            target_sha: claim().accepted_base,
        };
        assert!(fixture.create_ref(&command).is_err());
    }

    #[test]
    fn object_id_validation_rejects_abbreviations() {
        assert!(validate_object_id("deadbee", "base").is_err());
        assert!(validate_object_id("0123456789ABCDEF0123456789ABCDEF01234567", "base").is_ok());
        assert!(validate_object_id("0123456789abcdef0123456789abcdef0123456", "base").is_err());
    }

    #[test]
    fn receipt_vocabulary_covers_the_full_closed_set() {
        let expected = [
            CandidateReservationResultV1::Reserved,
            CandidateReservationResultV1::ExistingMatchingReservation,
            CandidateReservationResultV1::ExistingMatchingCandidate,
            CandidateReservationResultV1::ConflictingReservation,
            CandidateReservationResultV1::ReservationStale,
            CandidateReservationResultV1::ValidationRejected,
            CandidateReservationResultV1::ProviderUnavailable,
            CandidateReservationResultV1::RateLimitedOrAbuseProtected,
            CandidateReservationResultV1::InstrumentFailure,
            CandidateReservationResultV1::NotProven,
        ];
        for (index, first) in expected.iter().enumerate() {
            for second in expected.iter().skip(index + 1) {
                assert_ne!(first, second);
            }
        }
    }
}
