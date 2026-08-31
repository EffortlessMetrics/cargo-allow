//! Three-product agent review packet compiler (#3976 PR B).
//!
//! This module compiles the cargo-suite review profile
//! ([`CargoSuiteReviewProfileV1`], #3976 PR A) plus caller-supplied candidate,
//! intent, proof, claim, falsifier, old-path, and builder-narrative facts into
//! one deterministic [`CompiledReviewPacketV1`] with human
//! ([`render_compiled_packet_markdown`]) and machine
//! ([`render_compiled_packet_json`]) renders of the same semantic rows
//! ([`packet_parity_rows`]).
//!
//! The compiler is a pure adapter over the profile contract: every input is a
//! typed, caller-supplied value; no model invocation, repository access,
//! filesystem, process, network, or GitHub mutation happens here, and no
//! review is executed. The captured `agent_review_packet.v1` fixture from PR A
//! stays the sole packet contract: compilation validates the profile and its
//! binding to the canonical captured fixture
//! ([`captured_review_schema_fixture`]) and never forks or redefines the
//! shared schema.
//!
//! Readiness laws (a packet is [`PacketReadinessV1::NotReady`] with explicit
//! reasons when any of these holds):
//!
//! - a required review lens has no review-map coverage,
//! - the intent guidance currentness is not current,
//! - any proof reference currentness is not current (stale or unknown
//!   evidence keeps the packet non-ready; it never disappears),
//! - no proof references were supplied at all,
//! - an established claim declares no evidence refs, or
//! - no falsifiers were supplied.
//!
//! Contradictory proofs (a `Contradicted` outcome, or `Passed` plus `Failed`
//! outcomes for one plan ref) never flatten into one status: every proof row
//! keeps its own outcome and currentness, and the contradictions surface as
//! explicit contradiction rows in both renders. The builder narrative is
//! carried as a reference string only; a builder summary can never establish
//! the proposition.
//!
//! Determinism law: packet identity and both renders sort every multi-valued
//! field, and no absolute paths or timestamps participate anywhere.
//! Currentness is caller-asserted, never measured. Identity canonicalization
//! reuses the profile's reserved-separator encoding, so every caller-supplied
//! string is rejected when it carries a C0 control character or DEL (see
//! [`reject_identity_control_characters`]); the same rejection keeps the
//! human-readable single-line renders well-formed.
//!
//! Lens-coverage note: the shared review-map row carries no lens column, so
//! the compiler defines review-map coverage uniformly — every required lens
//! draws on the reviewer questions assigned to required closure surfaces, and
//! a packet whose review map assigns no reviewer question to any required
//! closure surface marks every required lens `Missing`. Per-lens
//! differentiation must come from the shared contract, never from a private
//! extension invented here.
//!
//! Candidate identity note: [`CandidateIdentityInputV1`] is dependency-neutral.
//! In production the caller derives its release-channel and release-version
//! fields from the release identity authority in the `allow-report` crate; the
//! cargo-allow crate exercises exactly that mapping at dev scope in its
//! `review_packet_compiler_parity` test, so the intent family carries no
//! cargo-allow-family dependency edge.

use serde::{Deserialize, Serialize};

use crate::agentic_candidate::ClaimRefV1;
use crate::agentic_review_profile::{
    AGENT_REVIEW_PACKET_SCHEMA_V1, CargoSuiteReviewProfileV1, ClosureSurfaceV1, FIELD_SEPARATOR,
    LIST_SEPARATOR, ReviewMapEntryV1, SHARED_REVIEW_PACKET_AUTHORITY,
    captured_review_schema_fixture, reject_identity_control_characters,
};
use crate::error::stable_hash_hex;

/// Caller-supplied candidate identity facts the packet needs. In production
/// the release-channel and release-version fields derive from the
/// `allow-report` release identity authority at the cargo-allow dev-scope
/// parity seam; this module stays dependency-neutral.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateIdentityInputV1 {
    pub repository: String,
    /// Stable reference to the governing claim or issue (never the raw diff).
    pub claim_ref: String,
    pub candidate_release_channel: String,
    pub candidate_release_version: String,
    pub base_commit: String,
    pub head_commit: String,
    pub tree_digest: String,
    /// Stable reference to the diff summary, never the raw diff payload.
    pub diff_summary_ref: String,
}

/// Caller-asserted currentness of an evidence source. Nothing is measured
/// here: the caller asserts it, and anything short of current keeps the packet
/// non-ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentCurrentnessV1 {
    Current,
    Stale,
    Unknown,
}

impl IntentCurrentnessV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Unknown => "unknown",
        }
    }
}

/// Caller-asserted cargo-intent guidance evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEvidenceInputV1 {
    pub guidance_ref: String,
    pub guidance_generation: String,
    pub boundary_summary: String,
    /// Caller-asserted guidance result (for example Accepted, Rejected, or
    /// Partial); carried verbatim, never re-derived.
    pub result_summary: String,
    pub currentness: IntentCurrentnessV1,
}

/// Caller-asserted outcome of one proof line. Contradiction is its own
/// outcome and is never flattened into passed or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofOutcomeSummaryV1 {
    Passed,
    Failed,
    Contradicted,
    NotRun,
}

impl ProofOutcomeSummaryV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Contradicted => "contradicted",
            Self::NotRun => "not_run",
        }
    }
}

/// One proof/gate/provider evidence line with its asserted currentness and
/// any contradiction notes preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofEvidenceInputV1 {
    pub plan_ref: String,
    pub gate_ref: String,
    pub receipt_ref: String,
    pub provider_name: String,
    pub outcome: ProofOutcomeSummaryV1,
    pub currentness: IntentCurrentnessV1,
    /// Contradiction notes, preserved verbatim in the compiled packet.
    pub contradictions: Vec<String>,
}

/// One proposition the packet claims is established, with the evidence refs
/// that establish it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstablishedClaimV1 {
    pub statement: String,
    pub evidence_refs: Vec<String>,
}

/// One proposition the packet refuses to claim, with the exclusion reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotEstablishedClaimV1 {
    pub statement: String,
    pub exclusion_reason: String,
}

/// One falsifier with the control that would trip it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FalsifierV1 {
    pub description: String,
    pub control_ref: String,
}

/// Disposition of one pre-change path after the change lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OldPathStatusV1 {
    Retired,
    StillLive,
    Unexamined,
}

impl OldPathStatusV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Retired => "retired",
            Self::StillLive => "still_live",
            Self::Unexamined => "unexamined",
        }
    }
}

/// One old-path disposition row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OldPathDispositionV1 {
    pub path_description: String,
    pub status: OldPathStatusV1,
    pub controlling_ref: String,
}

/// Reference to the builder's narrative summary. The summary may be
/// referenced only: a builder telling the story of a change can never
/// establish the proposition (issue law), so the packet carries the pointer
/// and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderNarrativeRefV1 {
    pub reference: String,
}

/// Everything the compiler consumes: the PR A profile by value plus typed,
/// caller-supplied facts. Compilation is pure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketCompilationRequestV1 {
    pub profile: CargoSuiteReviewProfileV1,
    pub candidate: CandidateIdentityInputV1,
    pub intent: IntentEvidenceInputV1,
    pub proofs: Vec<ProofEvidenceInputV1>,
    pub established: Vec<EstablishedClaimV1>,
    pub not_established: Vec<NotEstablishedClaimV1>,
    pub falsifiers: Vec<FalsifierV1>,
    pub old_paths: Vec<OldPathDispositionV1>,
    pub builder_narrative: BuilderNarrativeRefV1,
}

/// Readiness verdict for the compiled packet. `NotReady` carries the explicit,
/// sorted reasons; a non-ready packet still compiles in full so stale,
/// contradicted, and missing evidence stays visible instead of disappearing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketReadinessV1 {
    ReadyForFormalReview,
    NotReady { reasons: Vec<String> },
}

impl PacketReadinessV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ReadyForFormalReview => "ready_for_formal_review",
            Self::NotReady { .. } => "not_ready",
        }
    }
}

/// Per-lens evidence status. A required lens is never dropped from the packet:
/// missing evidence leaves the lens listed as missing and the packet
/// non-ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensEvidenceStatusV1 {
    Current,
    Missing,
}

impl LensEvidenceStatusV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Missing => "missing",
        }
    }
}

/// Exact base/head/tree/diff source facts of the compiled candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeforeAfterFactsV1 {
    pub base_commit: String,
    pub head_commit: String,
    pub tree_digest: String,
    pub diff_summary_ref: String,
}

/// The compiled three-product review packet: schema refs, deterministic
/// identity, readiness verdict, and every semantic row the shared packet
/// contract requires, sorted for byte-stable renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledReviewPacketV1 {
    /// The captured shared packet contract id; the sole contract.
    pub packet_schema: String,
    /// Shared-schema generation carried by the validated profile.
    pub shared_schema_generation: String,
    /// Content-derived identity of the validated PR A profile.
    pub profile_identity: String,
    /// The profile's governing claim.
    pub claim: ClaimRefV1,
    /// Content-derived identity of the governing claim.
    pub claim_identity: String,
    /// Human-readable canonical candidate identity line; carries the release
    /// channel and version spellings supplied by the caller.
    pub candidate_identity: String,
    /// Exact base/head/tree/diff facts of the candidate.
    pub before_after: BeforeAfterFactsV1,
    /// Authority rows: shared contract authority, contract id, and identities.
    pub authority_rows: Vec<String>,
    /// Required affected closure surfaces, sorted.
    pub affected_closure: Vec<ClosureSurfaceV1>,
    /// Established claims, sorted.
    pub established: Vec<EstablishedClaimV1>,
    /// Refused (not-established) claims, sorted.
    pub not_established: Vec<NotEstablishedClaimV1>,
    /// Falsifiers, sorted.
    pub falsifiers: Vec<FalsifierV1>,
    /// Per-proof summaries with outcome, currentness, and verbatim
    /// contradictions, sorted.
    pub proof_summaries: Vec<ProofEvidenceInputV1>,
    /// Contradiction rows preserved verbatim, sorted.
    pub contradiction_rows: Vec<String>,
    /// Review map, sorted.
    pub review_map: Vec<ReviewMapEntryV1>,
    /// Old-path dispositions, sorted.
    pub old_paths: Vec<OldPathDispositionV1>,
    /// Builder narrative reference; reference only, never evidence.
    pub builder_narrative_ref: String,
    /// One status row per required lens, sorted by lens name.
    pub lens_evidence_status: Vec<(String, LensEvidenceStatusV1)>,
    /// Readiness verdict with sorted explicit reasons when not ready.
    pub readiness: PacketReadinessV1,
    /// Profile limitations, sorted.
    pub limitations: Vec<String>,
    /// Profile overflow refs, sorted.
    pub overflow_refs: Vec<String>,
    /// Profile claim boundary.
    pub claim_boundary: String,
    /// Deterministic content identity of the compiled packet.
    pub packet_identity: String,
}

fn checked_field(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} must be non-empty"));
    }
    reject_identity_control_characters(name, value)
}

fn validate_request_strings(request: &PacketCompilationRequestV1) -> Result<(), String> {
    checked_field("candidate repository", &request.candidate.repository)?;
    checked_field("candidate claim_ref", &request.candidate.claim_ref)?;
    checked_field(
        "candidate release channel",
        &request.candidate.candidate_release_channel,
    )?;
    checked_field(
        "candidate release version",
        &request.candidate.candidate_release_version,
    )?;
    checked_field("candidate base commit", &request.candidate.base_commit)?;
    checked_field("candidate head commit", &request.candidate.head_commit)?;
    checked_field("candidate tree digest", &request.candidate.tree_digest)?;
    checked_field(
        "candidate diff summary ref",
        &request.candidate.diff_summary_ref,
    )?;
    checked_field("intent guidance_ref", &request.intent.guidance_ref)?;
    checked_field(
        "intent guidance_generation",
        &request.intent.guidance_generation,
    )?;
    checked_field("intent boundary_summary", &request.intent.boundary_summary)?;
    checked_field("intent result_summary", &request.intent.result_summary)?;
    for proof in &request.proofs {
        checked_field("proof plan_ref", &proof.plan_ref)?;
        checked_field("proof gate_ref", &proof.gate_ref)?;
        checked_field("proof receipt_ref", &proof.receipt_ref)?;
        checked_field("proof provider_name", &proof.provider_name)?;
        for note in &proof.contradictions {
            checked_field("proof contradiction note", note)?;
        }
    }
    for claim in &request.established {
        checked_field("established claim statement", &claim.statement)?;
        for evidence in &claim.evidence_refs {
            checked_field("established claim evidence ref", evidence)?;
        }
    }
    for claim in &request.not_established {
        checked_field("not-established claim statement", &claim.statement)?;
        checked_field("not-established exclusion reason", &claim.exclusion_reason)?;
    }
    for falsifier in &request.falsifiers {
        checked_field("falsifier description", &falsifier.description)?;
        checked_field("falsifier control ref", &falsifier.control_ref)?;
    }
    for old_path in &request.old_paths {
        checked_field("old path description", &old_path.path_description)?;
        checked_field("old path controlling ref", &old_path.controlling_ref)?;
    }
    checked_field(
        "builder narrative reference",
        &request.builder_narrative.reference,
    )?;
    Ok(())
}

fn candidate_identity_string(candidate: &CandidateIdentityInputV1) -> String {
    [
        format!("repository={}", candidate.repository),
        format!("claim_ref={}", candidate.claim_ref),
        format!("release_channel={}", candidate.candidate_release_channel),
        format!("release_version={}", candidate.candidate_release_version),
        format!("base_commit={}", candidate.base_commit),
        format!("head_commit={}", candidate.head_commit),
        format!("tree_digest={}", candidate.tree_digest),
        format!("diff_summary_ref={}", candidate.diff_summary_ref),
    ]
    .join(" ")
}

/// Compile the three-product agent review packet. Validates the profile
/// against the PR A contract and its captured shared-schema binding, checks
/// every caller-supplied string, enforces the readiness laws, and returns the
/// deterministic packet. Contradictions and non-current evidence never abort
/// compilation: they compile into visible rows and (where the laws require)
/// not-ready reasons.
pub fn compile_review_packet(
    request: PacketCompilationRequestV1,
) -> Result<CompiledReviewPacketV1, String> {
    validate_request_strings(&request)?;
    let PacketCompilationRequestV1 {
        profile,
        candidate,
        intent,
        proofs,
        established,
        not_established,
        falsifiers,
        old_paths,
        builder_narrative,
    } = request;
    profile
        .validate()
        .map_err(|error| format!("profile validation failed: {error}"))?;
    profile
        .bind_to_captured_schema(&captured_review_schema_fixture())
        .map_err(|error| format!("profile does not bind the captured shared schema: {error}"))?;
    if candidate.repository != profile.repository {
        return Err(format!(
            "candidate repository must match the profile repository {}, got {}",
            profile.repository, candidate.repository
        ));
    }
    let claim_identity = profile
        .claim
        .identity()
        .map_err(|error| format!("claim identity failed: {error}"))?;
    let profile_identity = profile
        .identity()
        .map_err(|error| format!("profile identity failed: {error}"))?;

    let review_map_covers_required_surfaces = profile.review_map.iter().any(|entry| {
        profile
            .required_closure_surfaces
            .iter()
            .any(|surface| surface.subject == entry.surface)
    });
    let lens_status = if review_map_covers_required_surfaces {
        LensEvidenceStatusV1::Current
    } else {
        LensEvidenceStatusV1::Missing
    };
    let mut lens_evidence_status: Vec<(String, LensEvidenceStatusV1)> = profile
        .required_lenses
        .iter()
        .map(|lens| (lens.as_str().to_string(), lens_status))
        .collect();
    lens_evidence_status.sort();

    let mut reasons: Vec<String> = Vec::new();
    if !review_map_covers_required_surfaces {
        for lens in &profile.required_lenses {
            reasons.push(format!(
                "required lens '{}' has no review-map coverage; no reviewer question is \
                 assigned to any required closure surface",
                lens.as_str()
            ));
        }
    }
    if intent.currentness != IntentCurrentnessV1::Current {
        reasons.push(format!(
            "intent guidance '{}' is {} and cannot establish currentness; stale or unknown \
             evidence keeps the packet non-ready",
            intent.guidance_ref,
            intent.currentness.as_str()
        ));
    }
    for proof in &proofs {
        if proof.currentness != IntentCurrentnessV1::Current {
            reasons.push(format!(
                "proof plan '{}' is {} and cannot establish currentness; stale or unknown \
                 evidence keeps the packet non-ready",
                proof.plan_ref,
                proof.currentness.as_str()
            ));
        }
    }
    if proofs.is_empty() {
        reasons.push(
            "no proof references were supplied; the required proof obligations have no \
             current evidence"
                .into(),
        );
    }
    for claim in &established {
        if claim.evidence_refs.is_empty() {
            reasons.push(format!(
                "established claim '{}' declares no evidence refs",
                claim.statement
            ));
        }
    }
    if falsifiers.is_empty() {
        reasons.push(
            "no falsifiers were supplied; a packet without falsifiers is not ready for \
             formal review"
                .into(),
        );
    }
    reasons.sort();
    reasons.dedup();
    let readiness = if reasons.is_empty() {
        PacketReadinessV1::ReadyForFormalReview
    } else {
        PacketReadinessV1::NotReady { reasons }
    };

    let mut contradiction_rows: Vec<String> = Vec::new();
    for proof in &proofs {
        for note in &proof.contradictions {
            contradiction_rows.push(format!(
                "proof plan '{}' contradiction: {}",
                proof.plan_ref, note
            ));
        }
    }
    for proof in &proofs {
        if proof.outcome == ProofOutcomeSummaryV1::Contradicted {
            contradiction_rows.push(format!(
                "proof plan '{}' outcome is contradicted",
                proof.plan_ref
            ));
        }
    }
    let mut plan_refs: Vec<&str> = proofs.iter().map(|proof| proof.plan_ref.as_str()).collect();
    plan_refs.sort_unstable();
    plan_refs.dedup();
    for plan_ref in plan_refs {
        let has_passed = proofs.iter().any(|proof| {
            proof.plan_ref == plan_ref && proof.outcome == ProofOutcomeSummaryV1::Passed
        });
        let has_failed = proofs.iter().any(|proof| {
            proof.plan_ref == plan_ref && proof.outcome == ProofOutcomeSummaryV1::Failed
        });
        if has_passed && has_failed {
            contradiction_rows.push(format!(
                "proof plan '{}' carries both passed and failed outcomes",
                plan_ref
            ));
        }
    }
    contradiction_rows.sort();

    let mut affected_closure = profile.required_closure_surfaces.clone();
    affected_closure.sort_by(|left, right| {
        (left.kind.as_str(), &left.subject).cmp(&(right.kind.as_str(), &right.subject))
    });
    let mut established_rows = established;
    established_rows.sort_by(|left, right| {
        (&left.statement, &left.evidence_refs).cmp(&(&right.statement, &right.evidence_refs))
    });
    let mut not_established_rows = not_established;
    not_established_rows.sort_by(|left, right| {
        (&left.statement, &left.exclusion_reason).cmp(&(&right.statement, &right.exclusion_reason))
    });
    let mut falsifier_rows = falsifiers;
    falsifier_rows.sort_by(|left, right| {
        (&left.description, &left.control_ref).cmp(&(&right.description, &right.control_ref))
    });
    let mut proof_summaries = proofs;
    proof_summaries.sort_by(|left, right| {
        (
            &left.plan_ref,
            &left.gate_ref,
            &left.receipt_ref,
            &left.provider_name,
        )
            .cmp(&(
                &right.plan_ref,
                &right.gate_ref,
                &right.receipt_ref,
                &right.provider_name,
            ))
    });
    let mut review_map = profile.review_map.clone();
    review_map.sort_by(|left, right| {
        (&left.surface, &left.reviewer_question).cmp(&(&right.surface, &right.reviewer_question))
    });
    let mut old_path_rows = old_paths;
    old_path_rows.sort_by(|left, right| {
        (&left.path_description, &left.controlling_ref)
            .cmp(&(&right.path_description, &right.controlling_ref))
    });
    let mut limitations = profile.limitations.clone();
    limitations.sort();
    let mut overflow_refs = profile.overflow_refs.clone();
    overflow_refs.sort();

    let mut authority_rows = vec![
        format!("captured shared packet contract authority: {SHARED_REVIEW_PACKET_AUTHORITY}"),
        format!(
            "packet contract: {AGENT_REVIEW_PACKET_SCHEMA_V1} (captured fixture; sole contract)"
        ),
        format!("claim identity: {claim_identity}"),
        format!("profile identity: {profile_identity}"),
    ];
    authority_rows.sort();

    let mut packet = CompiledReviewPacketV1 {
        packet_schema: AGENT_REVIEW_PACKET_SCHEMA_V1.into(),
        shared_schema_generation: profile.shared_schema_generation.clone(),
        profile_identity,
        claim: profile.claim.clone(),
        claim_identity,
        candidate_identity: candidate_identity_string(&candidate),
        before_after: BeforeAfterFactsV1 {
            base_commit: candidate.base_commit,
            head_commit: candidate.head_commit,
            tree_digest: candidate.tree_digest,
            diff_summary_ref: candidate.diff_summary_ref,
        },
        authority_rows,
        affected_closure,
        established: established_rows,
        not_established: not_established_rows,
        falsifiers: falsifier_rows,
        proof_summaries,
        contradiction_rows,
        review_map,
        old_paths: old_path_rows,
        builder_narrative_ref: builder_narrative.reference,
        lens_evidence_status,
        readiness,
        limitations,
        overflow_refs,
        claim_boundary: profile.claim_boundary.clone(),
        packet_identity: String::new(),
    };
    packet.packet_identity = packet.identity();
    Ok(packet)
}

impl CompiledReviewPacketV1 {
    /// Canonical identity payload of the compiled packet: every multi-valued
    /// field is sorted and joined with the profile's reserved separators, so
    /// the same semantic inputs always produce the same payload regardless of
    /// caller ordering. The stored packet identity itself does not
    /// participate (identity covers content, not itself).
    pub fn canonical_payload(&self) -> String {
        let readiness_segment = match &self.readiness {
            PacketReadinessV1::ReadyForFormalReview => "ready_for_formal_review".to_string(),
            PacketReadinessV1::NotReady { reasons } => {
                let mut sorted = reasons.clone();
                sorted.sort();
                format!(
                    "not_ready{}{}",
                    FIELD_SEPARATOR,
                    sorted.join(LIST_SEPARATOR)
                )
            }
        };
        let mut sorted_authority = self.authority_rows.clone();
        sorted_authority.sort();
        let mut closure_rows: Vec<String> = self
            .affected_closure
            .iter()
            .map(|surface| {
                [
                    surface.kind.as_str(),
                    surface.subject.as_str(),
                    surface.inclusion_reason.as_str(),
                ]
                .join(FIELD_SEPARATOR)
            })
            .collect();
        closure_rows.sort();
        let mut established_rows: Vec<String> = self
            .established
            .iter()
            .map(|claim| {
                [
                    claim.statement.as_str(),
                    claim.evidence_refs.join(LIST_SEPARATOR).as_str(),
                ]
                .join(FIELD_SEPARATOR)
            })
            .collect();
        established_rows.sort();
        let mut not_established_rows: Vec<String> = self
            .not_established
            .iter()
            .map(|claim| {
                [claim.statement.as_str(), claim.exclusion_reason.as_str()].join(FIELD_SEPARATOR)
            })
            .collect();
        not_established_rows.sort();
        let mut falsifier_rows: Vec<String> = self
            .falsifiers
            .iter()
            .map(|falsifier| {
                [
                    falsifier.description.as_str(),
                    falsifier.control_ref.as_str(),
                ]
                .join(FIELD_SEPARATOR)
            })
            .collect();
        falsifier_rows.sort();
        let mut proof_rows: Vec<String> = self
            .proof_summaries
            .iter()
            .map(|proof| {
                [
                    proof.plan_ref.as_str(),
                    proof.gate_ref.as_str(),
                    proof.receipt_ref.as_str(),
                    proof.provider_name.as_str(),
                    proof.outcome.as_str(),
                    proof.currentness.as_str(),
                    proof.contradictions.join(LIST_SEPARATOR).as_str(),
                ]
                .join(FIELD_SEPARATOR)
            })
            .collect();
        proof_rows.sort();
        let mut review_map_rows: Vec<String> = self
            .review_map
            .iter()
            .map(|entry| {
                [entry.surface.as_str(), entry.reviewer_question.as_str()].join(FIELD_SEPARATOR)
            })
            .collect();
        review_map_rows.sort();
        let mut old_path_rows: Vec<String> = self
            .old_paths
            .iter()
            .map(|old_path| {
                [
                    old_path.path_description.as_str(),
                    old_path.status.as_str(),
                    old_path.controlling_ref.as_str(),
                ]
                .join(FIELD_SEPARATOR)
            })
            .collect();
        old_path_rows.sort();
        let mut lens_rows: Vec<String> = self
            .lens_evidence_status
            .iter()
            .map(|(lens, status)| [lens.as_str(), status.as_str()].join(FIELD_SEPARATOR))
            .collect();
        lens_rows.sort();
        let mut contradiction_rows = self.contradiction_rows.clone();
        contradiction_rows.sort();
        let mut limitations = self.limitations.clone();
        limitations.sort();
        let mut overflow_refs = self.overflow_refs.clone();
        overflow_refs.sort();
        [
            AGENT_REVIEW_PACKET_SCHEMA_V1.to_string(),
            self.shared_schema_generation.clone(),
            self.profile_identity.clone(),
            self.claim_identity.clone(),
            self.candidate_identity.clone(),
            readiness_segment,
            self.before_after.base_commit.clone(),
            self.before_after.head_commit.clone(),
            self.before_after.tree_digest.clone(),
            self.before_after.diff_summary_ref.clone(),
            sorted_authority.join(LIST_SEPARATOR),
            closure_rows.join(LIST_SEPARATOR),
            established_rows.join(LIST_SEPARATOR),
            not_established_rows.join(LIST_SEPARATOR),
            falsifier_rows.join(LIST_SEPARATOR),
            proof_rows.join(LIST_SEPARATOR),
            review_map_rows.join(LIST_SEPARATOR),
            old_path_rows.join(LIST_SEPARATOR),
            lens_rows.join(LIST_SEPARATOR),
            contradiction_rows.join(LIST_SEPARATOR),
            self.builder_narrative_ref.clone(),
            limitations.join(LIST_SEPARATOR),
            overflow_refs.join(LIST_SEPARATOR),
            self.claim_boundary.clone(),
        ]
        .join(FIELD_SEPARATOR)
    }

    /// Deterministic content identity of the compiled packet. No timestamps,
    /// branch names, absolute paths, or runtime metadata participate.
    pub fn identity(&self) -> String {
        stable_hash_hex(&self.canonical_payload())
    }
}

/// The parity rows of the compiled packet: every semantic row the packet
/// carries, as `(section, row)` pairs in a fixed section order. Both renders
/// are built from exactly these rows, so the human and machine renders carry
/// the same semantics by construction — claim, authority, before/after,
/// closure, established, not-established, falsifiers, proofs (including
/// contradictions and currentness), review map, old paths, lenses, builder
/// reference, readiness with reasons, and the limitations/overflow/claim
/// boundary block.
pub fn packet_parity_rows(packet: &CompiledReviewPacketV1) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    rows.push((
        "claim".into(),
        format!("repository: {}", packet.claim.repository),
    ));
    rows.push((
        "claim".into(),
        format!("claim statement: {}", packet.claim.claim),
    ));
    rows.push((
        "claim".into(),
        format!("claim identity: {}", packet.claim_identity),
    ));
    rows.push((
        "authority".into(),
        format!("candidate identity: {}", packet.candidate_identity),
    ));
    for authority in &packet.authority_rows {
        rows.push(("authority".into(), authority.clone()));
    }
    rows.push((
        "before_after".into(),
        format!("base_commit: {}", packet.before_after.base_commit),
    ));
    rows.push((
        "before_after".into(),
        format!("head_commit: {}", packet.before_after.head_commit),
    ));
    rows.push((
        "before_after".into(),
        format!("tree_digest: {}", packet.before_after.tree_digest),
    ));
    rows.push((
        "before_after".into(),
        format!("diff_summary_ref: {}", packet.before_after.diff_summary_ref),
    ));
    for surface in &packet.affected_closure {
        rows.push((
            "affected_closure".into(),
            format!(
                "{} surface {}: {}",
                surface.kind.as_str(),
                surface.subject,
                surface.inclusion_reason
            ),
        ));
    }
    for claim in &packet.established {
        rows.push((
            "established".into(),
            format!(
                "{} (evidence: {})",
                claim.statement,
                claim.evidence_refs.join("; ")
            ),
        ));
    }
    for claim in &packet.not_established {
        rows.push((
            "not_established".into(),
            format!(
                "{} (exclusion: {})",
                claim.statement, claim.exclusion_reason
            ),
        ));
    }
    for falsifier in &packet.falsifiers {
        rows.push((
            "falsifiers".into(),
            format!(
                "{} (control: {})",
                falsifier.description, falsifier.control_ref
            ),
        ));
    }
    for proof in &packet.proof_summaries {
        rows.push((
            "proof".into(),
            format!(
                "plan {} gate {} receipt {} provider {} outcome {} currentness {}",
                proof.plan_ref,
                proof.gate_ref,
                proof.receipt_ref,
                proof.provider_name,
                proof.outcome.as_str(),
                proof.currentness.as_str()
            ),
        ));
    }
    for contradiction in &packet.contradiction_rows {
        rows.push(("proof".into(), format!("contradiction: {contradiction}")));
    }
    for entry in &packet.review_map {
        rows.push((
            "review_map".into(),
            format!("{} -> {}", entry.surface, entry.reviewer_question),
        ));
    }
    for old_path in &packet.old_paths {
        rows.push((
            "old_paths".into(),
            format!(
                "{} (status: {}, controlling ref: {})",
                old_path.path_description,
                old_path.status.as_str(),
                old_path.controlling_ref
            ),
        ));
    }
    for (lens, status) in &packet.lens_evidence_status {
        rows.push(("lenses".into(), format!("{lens}: {}", status.as_str())));
    }
    rows.push((
        "builder_narrative".into(),
        format!(
            "builder narrative reference: {} (reference only; a builder summary can never \
             establish the proposition)",
            packet.builder_narrative_ref
        ),
    ));
    rows.push((
        "readiness".into(),
        format!("packet identity: {}", packet.packet_identity),
    ));
    rows.push((
        "readiness".into(),
        format!("readiness: {}", packet.readiness.as_str()),
    ));
    if let PacketReadinessV1::NotReady { reasons } = &packet.readiness {
        for reason in reasons {
            rows.push(("readiness".into(), format!("not-ready reason: {reason}")));
        }
    }
    for limitation in &packet.limitations {
        rows.push((
            "limitations_overflow_claim_boundary".into(),
            format!("limitation: {limitation}"),
        ));
    }
    for reference in &packet.overflow_refs {
        rows.push((
            "limitations_overflow_claim_boundary".into(),
            format!("overflow ref: {reference}"),
        ));
    }
    rows.push((
        "limitations_overflow_claim_boundary".into(),
        format!("claim boundary: {}", packet.claim_boundary),
    ));
    rows
}

#[derive(Debug, Serialize)]
struct ParitySectionRowsV1 {
    section: String,
    rows: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PacketJsonRenderV1<'packet> {
    render_schema: &'static str,
    packet: &'packet CompiledReviewPacketV1,
    parity_sections: Vec<ParitySectionRowsV1>,
}

/// Render the compiled packet as pretty JSON. The render envelope carries the
/// full packet contract plus the parity sections, so the machine render
/// exposes exactly the same sectioned rows as the Markdown render.
pub fn render_compiled_packet_json(packet: &CompiledReviewPacketV1) -> Result<String, String> {
    let mut parity_sections: Vec<ParitySectionRowsV1> = Vec::new();
    for (section, row) in packet_parity_rows(packet) {
        match parity_sections.last_mut() {
            Some(last) if last.section == section => last.rows.push(row),
            _ => parity_sections.push(ParitySectionRowsV1 {
                section,
                rows: vec![row],
            }),
        }
    }
    let render = PacketJsonRenderV1 {
        render_schema: "cargo-allow.compiled-review-packet-json-render.v1",
        packet,
        parity_sections,
    };
    serde_json::to_string_pretty(&render)
        .map_err(|error| format!("compiled packet JSON render failed: {error}"))
}

/// Render the compiled packet as human-readable Markdown. The sections and
/// rows are built from [`packet_parity_rows`], so the human render carries
/// exactly the same semantic rows as the JSON render.
pub fn render_compiled_packet_markdown(packet: &CompiledReviewPacketV1) -> String {
    let mut lines: Vec<String> = vec![
        format!("# Compiled agent review packet {}", packet.packet_identity),
        String::new(),
        format!(
            "Packet contract: {AGENT_REVIEW_PACKET_SCHEMA_V1} (captured fixture; sole contract)."
        ),
        String::new(),
    ];
    let mut current_section: Option<String> = None;
    for (section, row) in packet_parity_rows(packet) {
        match &current_section {
            Some(open) if *open == section => {}
            _ => {
                lines.push(format!("## {section}"));
                lines.push(String::new());
                current_section = Some(section);
            }
        }
        lines.push(format!("- {row}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAIM_STATEMENT: &str = "packet compiler keeps human and machine renders in parity";
    const FALSIFIER_DESCRIPTION: &str = "a parity row missing from one render";
    const VERBATIM_CONTRADICTION: &str = "provider disagreed with the receipt";

    fn compiler_profile() -> CargoSuiteReviewProfileV1 {
        CargoSuiteReviewProfileV1 {
            profile_schema: crate::agentic_review_profile::CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1
                .into(),
            repository: "EffortlessMetrics/cargo-allow".into(),
            claim: ClaimRefV1 {
                repository: "EffortlessMetrics/cargo-allow".into(),
                controlling_issue: 3976,
                change: "review-packet-compiler".into(),
                semantic_route: "intent-model.agentic_review_packet_compiler".into(),
                claim: CLAIM_STATEMENT.into(),
                writer_key: "review-packet-compiler".into(),
                accepted_base: "0123456789abcdef0123456789abcdef01234567".into(),
                claim_boundary: "compiled packet parity only".into(),
            },
            shared_schema_generation:
                crate::agentic_review_profile::CAPTURED_REVIEW_SCHEMA_GENERATION.into(),
            profile_generation: "cargo-suite-review-profile-generation-1".into(),
            adapter_generation: "review-packet-compiler-generation-1".into(),
            intent_boundary: "cargo-intent accepted change authority".into(),
            intent_result: "accepted".into(),
            claim_ceiling: "one reviewed semantic transition".into(),
            required_closure_surfaces: vec![ClosureSurfaceV1 {
                kind: crate::agentic_review_profile::ClosureSurfaceKindV1::Owned,
                subject: "crates/intent-model/src/agentic_review_packet_compiler.rs".into(),
                inclusion_reason: "compiler module under review".into(),
            }],
            required_proof_obligations: vec![
                crate::agentic_review_profile::ProofObligationKindV1::IntentGuidance,
                crate::agentic_review_profile::ProofObligationKindV1::ProofGate,
            ],
            required_lenses: vec![
                crate::agentic_review_profile::ReviewLensV1::SemanticCorrectness,
                crate::agentic_review_profile::ReviewLensV1::SubjectEvidenceIdentity,
            ],
            review_map: vec![ReviewMapEntryV1 {
                surface: "crates/intent-model/src/agentic_review_packet_compiler.rs".into(),
                reviewer_question: "does the compiler preserve contradictions and lens \
                                    coverage?"
                    .into(),
            }],
            limitations: vec!["no model invocation in this slice".into()],
            overflow_refs: vec!["EffortlessMetrics/cargo-allow#3976".into()],
            claim_boundary: "pure adapter compilation only".into(),
        }
    }

    fn candidate() -> CandidateIdentityInputV1 {
        CandidateIdentityInputV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            claim_ref: "EffortlessMetrics/cargo-allow#3976".into(),
            candidate_release_channel: "stable".into(),
            candidate_release_version: "0.2.0".into(),
            base_commit: "0123456789abcdef0123456789abcdef01234567".into(),
            head_commit: "89abcdef0123456789abcdef0123456789abcdef".into(),
            tree_digest: "tree-digest-fixture-0001".into(),
            diff_summary_ref: "diff-summary:3976-pr-b".into(),
        }
    }

    fn passed_proof(plan_ref: &str, contradictions: Vec<String>) -> ProofEvidenceInputV1 {
        ProofEvidenceInputV1 {
            plan_ref: plan_ref.into(),
            gate_ref: format!("gate-for-{plan_ref}"),
            receipt_ref: format!("receipt-for-{plan_ref}"),
            provider_name: "local-validation".into(),
            outcome: ProofOutcomeSummaryV1::Passed,
            currentness: IntentCurrentnessV1::Current,
            contradictions,
        }
    }

    fn request() -> PacketCompilationRequestV1 {
        PacketCompilationRequestV1 {
            profile: compiler_profile(),
            candidate: candidate(),
            intent: IntentEvidenceInputV1 {
                guidance_ref: "intent-guidance:3976".into(),
                guidance_generation: "guidance-generation-1".into(),
                boundary_summary: "compiler module only".into(),
                result_summary: "Accepted".into(),
                currentness: IntentCurrentnessV1::Current,
            },
            proofs: vec![
                passed_proof("proof-plan:3976-b", Vec::new()),
                ProofEvidenceInputV1 {
                    plan_ref: "proof-plan:3976-parity".into(),
                    gate_ref: "proof-gate:3976-parity".into(),
                    receipt_ref: "proof-receipt:3976-parity".into(),
                    provider_name: "local-validation".into(),
                    outcome: ProofOutcomeSummaryV1::Passed,
                    currentness: IntentCurrentnessV1::Current,
                    contradictions: Vec::new(),
                },
            ],
            established: vec![
                EstablishedClaimV1 {
                    statement: "readiness laws keep stale evidence visible".into(),
                    evidence_refs: vec!["proof-receipt-for-stale-law".into()],
                },
                EstablishedClaimV1 {
                    statement: "renders carry the claim statement".into(),
                    evidence_refs: vec!["proof-receipt:3976-parity".into()],
                },
            ],
            not_established: vec![NotEstablishedClaimV1 {
                statement: "macro-expanded behavior is proven".into(),
                exclusion_reason: "outside the syntax-visible scan boundary".into(),
            }],
            falsifiers: vec![
                FalsifierV1 {
                    description: FALSIFIER_DESCRIPTION.into(),
                    control_ref: "parity-test:3976-b".into(),
                },
                FalsifierV1 {
                    description: "a contradictory proof flattened to one status".into(),
                    control_ref: "contradiction-test:3976-b".into(),
                },
            ],
            old_paths: vec![
                OldPathDispositionV1 {
                    path_description: "manual packet assembly in review notes".into(),
                    status: OldPathStatusV1::Retired,
                    controlling_ref: "EffortlessMetrics/cargo-allow#3976".into(),
                },
                OldPathDispositionV1 {
                    path_description: "ad hoc human-only review summaries".into(),
                    status: OldPathStatusV1::StillLive,
                    controlling_ref: "EffortlessMetrics/cargo-allow#3976".into(),
                },
            ],
            builder_narrative: BuilderNarrativeRefV1 {
                reference: "builder-summary:3976-pr-b".into(),
            },
        }
    }

    #[test]
    fn narrow_bug_request_compiles_ready_with_parity_renders() -> Result<(), String> {
        let packet = compile_review_packet(request())?;
        assert_eq!(packet.readiness, PacketReadinessV1::ReadyForFormalReview);
        let json = render_compiled_packet_json(&packet)?;
        let markdown = render_compiled_packet_markdown(&packet);
        for needle in [packet.claim.claim.as_str(), FALSIFIER_DESCRIPTION] {
            assert!(
                json.contains(needle),
                "JSON render lost a semantic row: {needle}"
            );
            assert!(
                markdown.contains(needle),
                "Markdown render lost a semantic row: {needle}"
            );
        }
        Ok(())
    }

    #[test]
    fn reordered_semantic_inputs_produce_identical_identity_and_renders() -> Result<(), String> {
        let first = compile_review_packet(request())?;
        let mut second_request = request();
        second_request.proofs.reverse();
        second_request.established.reverse();
        second_request.falsifiers.reverse();
        second_request.old_paths.reverse();
        let second = compile_review_packet(second_request)?;
        assert_eq!(first.packet_identity, second.packet_identity);
        assert_eq!(
            render_compiled_packet_json(&first)?,
            render_compiled_packet_json(&second)?
        );
        assert_eq!(
            render_compiled_packet_markdown(&first),
            render_compiled_packet_markdown(&second)
        );
        Ok(())
    }

    #[test]
    fn stale_intent_is_not_ready_and_contradictions_never_flatten() -> Result<(), String> {
        let mut stale = request();
        stale.intent.currentness = IntentCurrentnessV1::Stale;
        let stale_packet = compile_review_packet(stale)?;
        let PacketReadinessV1::NotReady { reasons } = &stale_packet.readiness else {
            return Err("stale intent must produce a not-ready packet".into());
        };
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("stale") && reason.contains("intent guidance")),
            "the not-ready reasons must name intent staleness: {reasons:?}"
        );

        let mut contradicted = request();
        contradicted.proofs = vec![
            passed_proof(
                "proof-plan:3976-shared",
                vec![VERBATIM_CONTRADICTION.into()],
            ),
            ProofEvidenceInputV1 {
                plan_ref: "proof-plan:3976-shared".into(),
                gate_ref: "proof-gate:3976-shared".into(),
                receipt_ref: "proof-receipt:3976-shared-failed".into(),
                provider_name: "local-validation".into(),
                outcome: ProofOutcomeSummaryV1::Failed,
                currentness: IntentCurrentnessV1::Current,
                contradictions: Vec::new(),
            },
        ];
        let packet = compile_review_packet(contradicted)?;
        assert_eq!(
            packet.contradiction_rows,
            vec![
                "proof plan 'proof-plan:3976-shared' carries both passed and failed outcomes"
                    .to_string(),
                format!(
                    "proof plan 'proof-plan:3976-shared' contradiction: {VERBATIM_CONTRADICTION}"
                ),
            ]
        );
        let json = render_compiled_packet_json(&packet)?;
        let markdown = render_compiled_packet_markdown(&packet);
        for needle in [
            "carries both passed and failed outcomes",
            VERBATIM_CONTRADICTION,
            "outcome passed",
            "outcome failed",
        ] {
            assert!(
                json.contains(needle),
                "JSON render flattened or lost a contradiction row: {needle}"
            );
            assert!(
                markdown.contains(needle),
                "Markdown render flattened or lost a contradiction row: {needle}"
            );
        }
        Ok(())
    }

    #[test]
    fn uncovered_lens_stays_listed_missing_and_blocks_readiness() -> Result<(), String> {
        let mut uncovered = request();
        let entry = uncovered
            .profile
            .review_map
            .first_mut()
            .ok_or("expected a review map fixture")?;
        entry.surface = "somewhere/outside/the/required/closure".into();
        let packet = compile_review_packet(uncovered)?;
        let PacketReadinessV1::NotReady { reasons } = &packet.readiness else {
            return Err("an uncovered required lens must block readiness".into());
        };
        let expected_lenses: Vec<&str> = compiler_profile()
            .required_lenses
            .iter()
            .map(crate::agentic_review_profile::ReviewLensV1::as_str)
            .collect();
        let statuses: Vec<&(String, LensEvidenceStatusV1)> =
            packet.lens_evidence_status.iter().collect();
        if statuses.len() != expected_lenses.len() {
            return Err(format!(
                "every required lens must stay listed: {statuses:?}"
            ));
        }
        for (lens, status) in statuses {
            if !expected_lenses.contains(&lens.as_str()) {
                return Err(format!("unexpected lens row: {lens}"));
            }
            if *status != LensEvidenceStatusV1::Missing {
                return Err(format!("uncovered lens must be missing: {lens}"));
            }
        }
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("required lens")),
            "the not-ready reasons must name the lens coverage gap: {reasons:?}"
        );
        Ok(())
    }

    #[test]
    fn builder_narrative_is_reference_only_and_established_stays_caller_owned() -> Result<(), String>
    {
        let packet = compile_review_packet(request())?;
        let mut expected = request().established;
        expected.sort_by(|left, right| {
            (&left.statement, &left.evidence_refs).cmp(&(&right.statement, &right.evidence_refs))
        });
        assert_eq!(packet.established, expected);
        let markdown = render_compiled_packet_markdown(&packet);
        assert!(
            markdown.contains("builder-summary:3976-pr-b"),
            "the builder narrative reference must be carried"
        );
        let builder_row_prefix = "builder narrative reference:";
        for (section, row) in packet_parity_rows(&packet) {
            if row.starts_with(builder_row_prefix) {
                assert_eq!(section, "builder_narrative");
            }
            assert!(
                !row.contains("builder-summary:3976-pr-b") || section == "builder_narrative",
                "the builder reference must never leak into other sections: {section}"
            );
        }
        Ok(())
    }

    #[test]
    fn identity_injectivity_regressions_reject_collision_inputs() -> Result<(), String> {
        let mut collision = request();
        collision.profile.intent_boundary = "a\u{1f}b".into();
        let collision_error = compile_review_packet(collision)
            .err()
            .ok_or("expected separator-collision rejection")?;
        assert!(collision_error.contains("control characters"));

        let mut empty_map = request();
        empty_map.profile.review_map.clear();
        let map_error = compile_review_packet(empty_map)
            .err()
            .ok_or("expected empty review-map rejection")?;
        assert!(map_error.contains("assigns no reviewer question"));
        Ok(())
    }

    #[test]
    fn empty_falsifiers_is_not_ready_and_empty_review_map_is_rejected() -> Result<(), String> {
        let mut no_falsifiers = request();
        no_falsifiers.falsifiers.clear();
        let packet = compile_review_packet(no_falsifiers)?;
        let PacketReadinessV1::NotReady { reasons } = &packet.readiness else {
            return Err("a packet without falsifiers must not be ready".into());
        };
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("no falsifiers were supplied")),
            "the falsifier gap must be an explicit reason: {reasons:?}"
        );

        let mut empty_map = request();
        empty_map.profile.review_map.clear();
        let error = compile_review_packet(empty_map)
            .err()
            .ok_or("expected empty review-map rejection")?;
        assert!(error.contains("review_map must assign at least one reviewer question"));
        Ok(())
    }

    #[test]
    fn parity_rows_appear_in_both_renders() -> Result<(), String> {
        let packet = compile_review_packet(request())?;
        let rows = packet_parity_rows(&packet);
        if rows.is_empty() {
            return Err("parity rows must not be empty".into());
        }
        let json = render_compiled_packet_json(&packet)?;
        let markdown = render_compiled_packet_markdown(&packet);
        for (section, row) in rows {
            assert!(
                json.contains(&section) && json.contains(&row),
                "JSON render lost section {section} row: {row}"
            );
            assert!(
                markdown.contains(&section) && markdown.contains(&row),
                "Markdown render lost section {section} row: {row}"
            );
        }
        Ok(())
    }
}
