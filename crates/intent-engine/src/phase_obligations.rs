//! Phase obligation compile plans (#2586-C).
//!
//! These DTOs describe obligation structure derived from graph movements and
//! inventory posture. They do not invoke `allow-policy` evaluation or embed
//! provider argv, proof programs, or execution receipts.

use crate::graph_comparison::{GraphMovementKindV1, GraphMovementV1, sort_graph_movements};
use serde::{Deserialize, Serialize};

pub const PHASE_OBLIGATION_PLAN_SCHEMA_ID: &str = "intent.phase-obligation-plan.v1";
pub const PRECOMMIT_PHASE_ID: &str = "precommit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryPostureV1 {
    Complete,
    Partial,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseObligationKindV1 {
    EvidenceReview,
    ImplementationClosure,
    SupportClaimReview,
    InventoryCompleteness,
    SubjectResolution,
    PolicyAlignment,
}

impl PhaseObligationKindV1 {
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
pub enum ObligationPostureV1 {
    Blocking,
    Advisory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseObligationItemV1 {
    pub obligation_id: String,
    pub phase: String,
    pub kind: PhaseObligationKindV1,
    pub statement: String,
    pub posture: ObligationPostureV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseObligationPlanV1 {
    pub schema_id: String,
    pub phase: String,
    pub obligations: Vec<PhaseObligationItemV1>,
}

impl PhaseObligationPlanV1 {
    pub fn new(phase: impl Into<String>, obligations: Vec<PhaseObligationItemV1>) -> Self {
        Self {
            schema_id: PHASE_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
            phase: phase.into(),
            obligations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseObligationCompileInputV1 {
    pub phase: String,
    pub movements: Vec<GraphMovementV1>,
    pub inventory: InventoryPostureV1,
    pub legacy_baseline: bool,
}

/// Compile a phase obligation skeleton from normalized graph movements.
///
/// This is a contract compiler for transport and parity only. Authoritative
/// precommit findings remain in `allow-policy` until intent-engine cutover.
pub fn compile_phase_obligation_plan(
    input: &PhaseObligationCompileInputV1,
) -> PhaseObligationPlanV1 {
    let posture = if input.legacy_baseline {
        ObligationPostureV1::Advisory
    } else {
        ObligationPostureV1::Blocking
    };
    let mut obligations = Vec::new();
    let phase = input.phase.as_str();

    if input.inventory != InventoryPostureV1::Complete {
        obligations.push(obligation(
            "inventory-completeness",
            phase,
            PhaseObligationKindV1::InventoryCompleteness,
            "staged source inventory must be complete before exact subject obligations are current",
            posture,
        ));
    }

    let mut movements = input.movements.clone();
    sort_graph_movements(&mut movements);

    let mut policy_surface = false;
    let mut implementation_surface = false;
    let mut evidence_surface = false;
    let mut subject_surface = false;

    for movement in &movements {
        match movement.kind {
            GraphMovementKindV1::RequirementAdded
            | GraphMovementKindV1::RequirementRemoved
            | GraphMovementKindV1::RequirementChanged
            | GraphMovementKindV1::ProfileOrDialectChanged => {
                policy_surface = true;
            }
            GraphMovementKindV1::ImplementationSliceAdded
            | GraphMovementKindV1::ImplementationSliceRemoved
            | GraphMovementKindV1::ImplementationSliceChanged
            | GraphMovementKindV1::SeamMappingAdded
            | GraphMovementKindV1::SeamMappingRemoved
            | GraphMovementKindV1::SeamMappingChanged => {
                implementation_surface = true;
            }
            GraphMovementKindV1::EvidencePurposeAdded
            | GraphMovementKindV1::EvidencePurposeRemoved
            | GraphMovementKindV1::EvidencePurposeChanged
            | GraphMovementKindV1::EvidenceClaimChanged => {
                evidence_surface = true;
            }
            GraphMovementKindV1::SubjectSelectorAdded
            | GraphMovementKindV1::SubjectSelectorRemoved
            | GraphMovementKindV1::SubjectSelectorChanged
            | GraphMovementKindV1::SubjectBodyIdentityChanged => {
                subject_surface = true;
            }
            GraphMovementKindV1::UnknownOrUncomparable => {
                policy_surface = true;
            }
        }

        if movement.kind == GraphMovementKindV1::SubjectBodyIdentityChanged {
            obligations.push(obligation(
                &format!("evidence-review-{}", movement.id),
                phase,
                PhaseObligationKindV1::EvidenceReview,
                "exact test body identity changed and dependent evidence must be revalidated",
                ObligationPostureV1::Blocking,
            ));
        }
    }

    if policy_surface {
        obligations.push(obligation(
            "policy-alignment",
            phase,
            PhaseObligationKindV1::PolicyAlignment,
            "staged graph policy surface changed and declarations must remain aligned",
            posture,
        ));
    }
    if implementation_surface {
        obligations.push(obligation(
            "implementation-closure",
            phase,
            PhaseObligationKindV1::ImplementationClosure,
            "implementation slice or seam mapping changed and closure must be reviewed",
            posture,
        ));
    }
    if evidence_surface {
        obligations.push(obligation(
            "evidence-review",
            phase,
            PhaseObligationKindV1::EvidenceReview,
            "evidence claim surface changed and receipts must be reviewed",
            posture,
        ));
    }
    if subject_surface {
        obligations.push(obligation(
            "subject-resolution",
            phase,
            PhaseObligationKindV1::SubjectResolution,
            "subject selector or inventory binding changed and exact resolution must be current",
            posture,
        ));
    }

    obligations.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.obligation_id.cmp(&right.obligation_id))
    });

    PhaseObligationPlanV1::new(phase, obligations)
}

fn obligation(
    obligation_id: &str,
    phase: &str,
    kind: PhaseObligationKindV1,
    statement: &str,
    posture: ObligationPostureV1,
) -> PhaseObligationItemV1 {
    PhaseObligationItemV1 {
        obligation_id: obligation_id.to_string(),
        phase: phase.to_string(),
        kind,
        statement: statement.to_string(),
        posture,
    }
}

pub fn load_phase_obligation_plan_toml(text: &str) -> Result<PhaseObligationPlanV1, String> {
    toml::from_str(text).map_err(|err| format!("parse phase obligation plan: {err}"))
}
