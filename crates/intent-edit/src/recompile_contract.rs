//! Recompile obligation contract bound to intent-engine transport (#2613-E).
//!
//! Compiles post-edit recompile obligations aligned with
//! `intent-engine::phase_obligations` transport. Does not invoke graph
//! compilation, precommit evaluation, or proof commands.

use intent_engine::{
    PHASE_OBLIGATION_PLAN_SCHEMA_ID, PRECOMMIT_PHASE_ID, PhaseObligationKindV1,
    PhaseObligationPlanV1,
};

use crate::repo_edit_translation::RepoEditTranslationPlanV1;

pub const INTENT_EDIT_RECOMPILE_CONTRACT_SCHEMA_ID: &str = "intent.edit-recompile-contract.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEditRecompileObligationV1 {
    pub obligation_id: String,
    pub phase: String,
    pub kind: PhaseObligationKindV1,
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

impl IntentEditRecompileContractV1 {
    pub fn to_phase_obligation_plan(&self) -> PhaseObligationPlanV1 {
        PhaseObligationPlanV1::new(
            PRECOMMIT_PHASE_ID,
            self.obligations
                .iter()
                .map(|item| intent_engine::PhaseObligationItemV1 {
                    obligation_id: item.obligation_id.clone(),
                    phase: item.phase.clone(),
                    kind: item.kind,
                    statement: item.statement.clone(),
                    posture: intent_engine::ObligationPostureV1::Blocking,
                })
                .collect(),
        )
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
            kind: PhaseObligationKindV1::PolicyAlignment,
            statement: format!("recompile candidate graph after edit to {}", request.target),
            action_id: request.action_id.clone(),
        });
    }
    IntentEditRecompileContractV1 {
        schema_id: INTENT_EDIT_RECOMPILE_CONTRACT_SCHEMA_ID.to_string(),
        plan_id: translation.plan_id.clone(),
        target_transport_schema_id: PHASE_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
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
