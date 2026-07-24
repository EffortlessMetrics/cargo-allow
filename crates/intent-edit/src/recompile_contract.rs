//! Recompile obligation contract bound to intent-engine transport (#2613-E).
//!
//! Compiles post-edit recompile obligations aligned with
//! `intent-engine::phase_obligations` transport. Does not invoke graph
//! compilation, precommit evaluation, or proof commands.

use serde::{Deserialize, Serialize};

use crate::repo_edit_translation::RepoEditTranslationPlanV1;

pub const INTENT_EDIT_RECOMPILE_CONTRACT_SCHEMA_ID: &str = "intent.edit-recompile-contract.v1";
pub const TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID: &str = "intent.phase-obligation-plan.v1";
pub const PRECOMMIT_PHASE_ID: &str = "precommit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecompileObligationKindV1 {
    PolicyAlignment,
}

impl RecompileObligationKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PolicyAlignment => "policy_alignment",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEditRecompileObligationV1 {
    pub obligation_id: String,
    pub phase: String,
    pub kind: RecompileObligationKindV1,
    pub statement: String,
    pub action_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEditRecompileContractV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub target_transport_schema_id: String,
    pub obligations: Vec<IntentEditRecompileObligationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseObligationTransportPostureV1 {
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseObligationTransportItemV1 {
    pub obligation_id: String,
    pub phase: String,
    pub kind: RecompileObligationKindV1,
    pub statement: String,
    pub posture: PhaseObligationTransportPostureV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseObligationTransportPlanV1 {
    pub schema_id: String,
    pub phase: String,
    pub obligations: Vec<PhaseObligationTransportItemV1>,
}

impl IntentEditRecompileContractV1 {
    pub fn to_phase_obligation_transport_plan(&self) -> PhaseObligationTransportPlanV1 {
        PhaseObligationTransportPlanV1 {
            schema_id: TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
            phase: PRECOMMIT_PHASE_ID.to_string(),
            obligations: self
                .obligations
                .iter()
                .map(|item| PhaseObligationTransportItemV1 {
                    obligation_id: item.obligation_id.clone(),
                    phase: item.phase.clone(),
                    kind: item.kind,
                    statement: item.statement.clone(),
                    posture: PhaseObligationTransportPostureV1::Blocking,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecompileContractError {
    PlanIdMismatch { expected: String, observed: String },
    NoObligations,
}

impl RecompileContractError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlanIdMismatch { .. } => "plan_id_mismatch",
            Self::NoObligations => "no_obligations",
        }
    }
}

pub fn compile_recompile_contract(
    translation: &RepoEditTranslationPlanV1,
) -> IntentEditRecompileContractV1 {
    let mut obligations = Vec::new();
    for request in &translation.requests {
        if !requires_recompile(&request.target) {
            continue;
        }
        obligations.push(IntentEditRecompileObligationV1 {
            obligation_id: format!("recompile:{}", request.action_id),
            phase: PRECOMMIT_PHASE_ID.to_string(),
            kind: RecompileObligationKindV1::PolicyAlignment,
            statement: format!("recompile candidate graph after edit to {}", request.target),
            action_id: request.action_id.clone(),
        });
    }
    IntentEditRecompileContractV1 {
        schema_id: INTENT_EDIT_RECOMPILE_CONTRACT_SCHEMA_ID.to_string(),
        plan_id: translation.plan_id.clone(),
        target_transport_schema_id: TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
        obligations,
    }
}

pub fn validate_recompile_contract(
    translation: &RepoEditTranslationPlanV1,
    contract: &IntentEditRecompileContractV1,
) -> Result<(), RecompileContractError> {
    if translation.plan_id != contract.plan_id {
        return Err(RecompileContractError::PlanIdMismatch {
            expected: translation.plan_id.clone(),
            observed: contract.plan_id.clone(),
        });
    }
    let expected_count = translation
        .requests
        .iter()
        .filter(|request| requires_recompile(&request.target))
        .count();
    if expected_count != contract.obligations.len() {
        return Err(RecompileContractError::NoObligations);
    }
    Ok(())
}

fn requires_recompile(target: &str) -> bool {
    target.starts_with("policy/") || target.starts_with(".allow/") || target.contains("/allow.toml")
}
