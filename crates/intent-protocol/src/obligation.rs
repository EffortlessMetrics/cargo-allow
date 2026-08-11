//! Intent phase-obligation plan transport envelopes (#2585-C).
//!
//! These DTOs describe obligation structure only. They do not embed provider
//! argv, proof programs, RIPR/Hawk dialect enums, or execution receipts.

use crate::identity::IntentIdentityEnvelopeV1;
use effortless_repo_protocol::ResultClassV1;
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentObligationPlanEnvelopeV1 {
    pub schema_id: String,
    pub identity: IntentIdentityEnvelopeV1,
    pub phase: String,
    pub obligations: Vec<IntentPhaseObligationV1>,
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
