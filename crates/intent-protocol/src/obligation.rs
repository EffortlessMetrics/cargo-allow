//! Intent phase-obligation plan transport envelopes (#2585-C).
//!
//! These DTOs describe obligation structure only. They do not embed provider
//! argv, proof programs, RIPR/Hawk dialect enums, or execution receipts.

use crate::identity::IntentIdentityEnvelopeV1;
use crate::snapshot_package::repo_protocol::ResultClassV1;
use serde::{Deserialize, Serialize};

pub const INTENT_OBLIGATION_PLAN_SCHEMA_ID: &str = "intent.obligation-plan.v1";
pub const INTENT_OBLIGATION_PLAN_RESPONSE_SCHEMA_ID: &str = "intent.obligation-plan-response.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPhaseObligationKindV1 {
    EvidenceReview,
    ImplementationClosure,
    SupportClaimReview,
    InventoryCompleteness,
    SubjectResolution,
    PolicyAlignment,
}

impl IntentPhaseObligationKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceReview => "evidence_review",
            Self::ImplementationClosure => "implementation_closure",
            Self::SupportClaimReview => "support_claim_review",
            Self::InventoryCompleteness => "inventory_completeness",
            Self::SubjectResolution => "subject_resolution",
            Self::PolicyAlignment => "policy_alignment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentObligationPostureV1 {
    Blocking,
    Advisory,
    /// A repository decision currently gates the obligation (#3964). No
    /// current producer emits this value yet; it is transported so decision
    /// posture never has to collapse into blocking or advisory by omission.
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentSubjectPostureV1 {
    Exact,
    Weak,
    Ambiguous,
    Missing,
    ZeroSubject,
}

impl IntentSubjectPostureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Weak => "weak",
            Self::Ambiguous => "ambiguous",
            Self::Missing => "missing",
            Self::ZeroSubject => "zero_subject",
        }
    }
}

/// One explicit per-obligation handoff disposition (#3964). Every applicable
/// evidence obligation carries exactly one of these; absence of the whole
/// handoff block means the producer predates the enrichment and can never be
/// interpreted as ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentProofHandoffDispositionV1 {
    ReadyForProofPlanning,
    RepositoryDecisionRequired,
    EvidenceDesignIncomplete,
    SelectorMissingOrAmbiguous,
    ManualOrNativeOutstanding,
    UnsupportedEvidenceClass,
    NotApplicableWithReason,
    PartialOrNotProven,
}

impl IntentProofHandoffDispositionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadyForProofPlanning => "ready_for_proof_planning",
            Self::RepositoryDecisionRequired => "repository_decision_required",
            Self::EvidenceDesignIncomplete => "evidence_design_incomplete",
            Self::SelectorMissingOrAmbiguous => "selector_missing_or_ambiguous",
            Self::ManualOrNativeOutstanding => "manual_or_native_outstanding",
            Self::UnsupportedEvidenceClass => "unsupported_evidence_class",
            Self::NotApplicableWithReason => "not_applicable_with_reason",
            Self::PartialOrNotProven => "partial_or_not_proven",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEvidenceIndependenceV1 {
    Automated,
    ManualOutstanding,
    NativeOutstanding,
}

impl IntentEvidenceIndependenceV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automated => "automated",
            Self::ManualOutstanding => "manual_outstanding",
            Self::NativeOutstanding => "native_outstanding",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentSubjectInventoryCompletenessV1 {
    Complete,
    Partial,
    Unknown,
}

impl IntentSubjectInventoryCompletenessV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }
}

/// Row-level PreTest handoff enrichment (#3964 INTENT-PROOF-HANDOFF-1).
///
/// Every field is a stable reference or a typed posture: source snippets,
/// guidance prose, provider payloads, receipts, and logs stay outside the
/// envelope. Requested evidence/capability class is a semantic requirement,
/// never an observation that a provider implements it. All fields are
/// optional so historical V1 rows remain valid; absence is honest and
/// consumers must treat missing semantics as not-ready, never fabricate them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct IntentObligationHandoffV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<IntentProofHandoffDispositionV1>,
    /// Required non-empty for `NotApplicableWithReason`; optional elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition_reason: Option<String>,
    /// EvidenceIntent / evidence-purpose references (why the evidence exists).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence_purpose_refs: Vec<String>,
    /// Discriminator references that make positive and negative cases distinct.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub discriminator_refs: Vec<String>,
    /// Forbidden or alternate observable references.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub forbidden_or_alternate_observable_refs: Vec<String>,
    /// Counterfactual references where the evaluator selected them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub counterfactual_refs: Vec<String>,
    /// Reference to the precondition fact the evidence presumes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precondition_ref: Option<String>,
    /// Reference to the operation under test.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_ref: Option<String>,
    /// Reference to the expected observable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_observable_ref: Option<String>,
    /// Implementation seam reference the evidence is taken at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seam_ref: Option<String>,
    /// Owner/scope reference for the seam.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_scope_ref: Option<String>,
    /// Requirement identity this obligation derives from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_ref: Option<String>,
    /// Target identity (crate/module/item) the evidence binds to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    /// Subject/selector reference resolved by the evaluator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_selector_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_posture: Option<IntentSubjectPostureV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_inventory_completeness: Option<IntentSubjectInventoryCompletenessV1>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject_inventory_limitations: Vec<String>,
    /// Requested evidence/capability class as a semantic requirement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_evidence_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_threshold: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independence: Option<IntentEvidenceIndependenceV1>,
    /// What the evaluator structurally established for this row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub established: Vec<String>,
    /// What remains unproven and why.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unproven: Vec<String>,
    /// Load-bearing currentness dimensions for this row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub currentness_dimensions: Vec<String>,
    /// Retrieval/overflow references kept behind stable handles.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub retrieval_refs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_boundary: Option<String>,
    /// Evaluator-produced semantic digest over this row's handoff basis
    /// (source/config/compiler/policy/evaluation/purpose identity). The
    /// protocol transports it; it does not define the digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_digest: Option<String>,
    /// Exact source-subject identity basis (staged/worktree/committed stay
    /// distinct identities).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_identity: Option<String>,
}

impl IntentObligationHandoffV1 {
    /// Fail-closed completeness law for the handoff block (#3964): readiness
    /// is only claimable with exact subject posture, an evidence purpose, a
    /// requested evidence class, a subject selector, and nothing unproven.
    pub fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("disposition_reason", self.disposition_reason.as_deref()),
            ("claim_boundary", self.claim_boundary.as_deref()),
            ("semantic_digest", self.semantic_digest.as_deref()),
            ("source_identity", self.source_identity.as_deref()),
        ] {
            if let Some(value) = value
                && value.trim().is_empty()
            {
                return Err(format!("handoff {label} must be non-empty when present"));
            }
        }
        let ready =
            self.disposition == Some(IntentProofHandoffDispositionV1::ReadyForProofPlanning);
        if ready {
            if self.subject_posture != Some(IntentSubjectPostureV1::Exact) {
                return Err("ready_for_proof_planning requires exact subject posture".to_string());
            }
            if self.evidence_purpose_refs.is_empty() {
                return Err(
                    "ready_for_proof_planning requires at least one evidence purpose reference"
                        .to_string(),
                );
            }
            if self
                .requested_evidence_class
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(
                    "ready_for_proof_planning requires a requested evidence class".to_string(),
                );
            }
            if self
                .subject_selector_ref
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(
                    "ready_for_proof_planning requires a subject selector reference".to_string(),
                );
            }
            if !self.unproven.is_empty() {
                return Err("ready_for_proof_planning cannot carry unproven items".to_string());
            }
        }
        if matches!(
            self.independence,
            Some(IntentEvidenceIndependenceV1::ManualOutstanding)
                | Some(IntentEvidenceIndependenceV1::NativeOutstanding)
        ) && ready
        {
            return Err(
                "manual or native outstanding independence cannot be ready for proof planning"
                    .to_string(),
            );
        }
        if self.disposition == Some(IntentProofHandoffDispositionV1::NotApplicableWithReason)
            && self
                .disposition_reason
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err(
                "not_applicable_with_reason requires a non-empty disposition reason".to_string(),
            );
        }
        Ok(())
    }
}

/// Envelope-level PreTest enrichment (#3964). The plan's semantic identity
/// stays derived (`intent_obligation_plan_digest` over the exact envelope); a
/// self-asserted plan digest is deliberately not stored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct IntentPlanEnrichmentV1 {
    /// Producer-declared protocol generation of this enrichment (>= 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_generation: Option<u32>,
    /// IntentGuidanceResultV1 identity reference for the PreTest basis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_result_identity: Option<String>,
    /// ResolvedIntentConfigV1 identity reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_config_identity: Option<String>,
    /// Exact repository subject identity reference (V2-capable; the V1
    /// snapshot value remains in `identity.snapshot`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_subject_identity: Option<String>,
    /// PreparedChangeRef identity where the route selected a change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_change_identity: Option<String>,
    /// Requested semantic boundary (for example "pretest").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_semantic_boundary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_generation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_generation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_generation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligation_evaluation_generation: Option<String>,
}

impl IntentPlanEnrichmentV1 {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(generation) = self.protocol_generation
            && generation < 1
        {
            return Err("protocol_generation must be at least 1 when present".to_string());
        }
        for (label, value) in [
            (
                "guidance_result_identity",
                self.guidance_result_identity.as_deref(),
            ),
            (
                "resolved_config_identity",
                self.resolved_config_identity.as_deref(),
            ),
            (
                "repository_subject_identity",
                self.repository_subject_identity.as_deref(),
            ),
            (
                "prepared_change_identity",
                self.prepared_change_identity.as_deref(),
            ),
            (
                "requested_semantic_boundary",
                self.requested_semantic_boundary.as_deref(),
            ),
            ("compiler_generation", self.compiler_generation.as_deref()),
            ("query_generation", self.query_generation.as_deref()),
            ("policy_generation", self.policy_generation.as_deref()),
            (
                "obligation_evaluation_generation",
                self.obligation_evaluation_generation.as_deref(),
            ),
        ] {
            if let Some(value) = value
                && value.trim().is_empty()
            {
                return Err(format!("enrichment {label} must be non-empty when present"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentPhaseObligationV1 {
    pub obligation_id: String,
    pub phase: String,
    pub kind: IntentPhaseObligationKindV1,
    pub statement: String,
    pub posture: IntentObligationPostureV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// PreTest proof-handoff enrichment (#3964). Absence means the producer
    /// predates the enrichment; consumers must treat the row as not ready and
    /// never fabricate the missing semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<IntentObligationHandoffV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentObligationPlanEnvelopeV1 {
    pub schema_id: String,
    pub identity: IntentIdentityEnvelopeV1,
    pub phase: String,
    pub obligations: Vec<IntentPhaseObligationV1>,
    /// Plan-level PreTest enrichment (#3964). Absence keeps historical V1
    /// artifacts valid exactly as produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<IntentPlanEnrichmentV1>,
}

impl IntentObligationPlanEnvelopeV1 {
    pub fn new(
        identity: IntentIdentityEnvelopeV1,
        phase: impl Into<String>,
        obligations: Vec<IntentPhaseObligationV1>,
    ) -> Self {
        Self {
            schema_id: INTENT_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
            identity,
            phase: phase.into(),
            obligations,
            enrichment: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentObligationPlanResponseV1 {
    pub schema_id: String,
    pub plan: IntentObligationPlanEnvelopeV1,
    pub result_class: ResultClassV1,
    pub open_obligation_count: u32,
}

impl IntentObligationPlanResponseV1 {
    pub fn new(
        plan: IntentObligationPlanEnvelopeV1,
        result_class: ResultClassV1,
        open_obligation_count: u32,
    ) -> Self {
        Self {
            schema_id: INTENT_OBLIGATION_PLAN_RESPONSE_SCHEMA_ID.to_string(),
            plan,
            result_class,
            open_obligation_count,
        }
    }
}
