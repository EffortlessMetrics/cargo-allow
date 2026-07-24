//! Settlement and residual obligations for intent edit cutover (#2613-F).
//!
//! Combines repo-edit translation and recompile contracts into a portable
//! settlement plan with post-apply residual obligations. Does not invoke
//! `repo-edit::apply_single_target`, proof commands, or refresh snapshots.

use crate::approval_currentness::{
    ApprovalCurrentnessError, IntentEditApprovalCurrentnessV1, validate_approval_currentness,
};
use crate::dialect_adapter::IntentEditDialectV1;
use crate::edit_plan::{IntentEditPlanError, IntentEditPlanV1, validate_edit_plan};
use crate::recompile_contract::{
    IntentEditRecompileContractV1, RecompileContractError, compile_recompile_contract,
    validate_recompile_contract,
};
use crate::repo_edit_translation::{
    RepoEditTranslationError, RepoEditTranslationPlanV1, translate_plan_to_repo_edit,
};

pub const INTENT_EDIT_SETTLEMENT_PLAN_SCHEMA_ID: &str = "intent.edit-settlement-plan.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentEditResidualObligationKindV1 {
    AwaitApplyReceipt,
    AwaitRecompileProof,
    AwaitCurrentnessRefresh,
}

impl IntentEditResidualObligationKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitApplyReceipt => "await_apply_receipt",
            Self::AwaitRecompileProof => "await_recompile_proof",
            Self::AwaitCurrentnessRefresh => "await_currentness_refresh",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEditResidualObligationV1 {
    pub obligation_id: String,
    pub kind: IntentEditResidualObligationKindV1,
    pub action_id: Option<String>,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentEditSettlementPlanV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub translation: RepoEditTranslationPlanV1,
    pub recompile_contract: IntentEditRecompileContractV1,
    pub residual_obligations: Vec<IntentEditResidualObligationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementError {
    EditPlan(IntentEditPlanError),
    Approval(ApprovalCurrentnessError),
    Translation(RepoEditTranslationError),
    Recompile(RecompileContractError),
    PlanIdMismatch { expected: String, observed: String },
    InvalidSchemaId { observed: String },
    ResidualCountMismatch { expected: usize, observed: usize },
}

impl SettlementError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EditPlan(_) => "edit_plan_invalid",
            Self::Approval(_) => "approval_invalid",
            Self::Translation(_) => "translation_failed",
            Self::Recompile(_) => "recompile_contract_invalid",
            Self::PlanIdMismatch { .. } => "plan_id_mismatch",
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::ResidualCountMismatch { .. } => "residual_count_mismatch",
        }
    }
}

pub fn compile_settlement_plan(
    plan: &IntentEditPlanV1,
    approval: &IntentEditApprovalCurrentnessV1,
    dialect: IntentEditDialectV1,
) -> Result<IntentEditSettlementPlanV1, SettlementError> {
    validate_edit_plan(plan).map_err(SettlementError::EditPlan)?;
    validate_approval_currentness(approval).map_err(SettlementError::Approval)?;
    if plan.plan_id != approval.plan_id {
        return Err(SettlementError::PlanIdMismatch {
            expected: plan.plan_id.clone(),
            observed: approval.plan_id.clone(),
        });
    }

    let translation = translate_plan_to_repo_edit(plan, approval, dialect)
        .map_err(SettlementError::Translation)?;
    let recompile_contract = compile_recompile_contract(&translation);
    validate_recompile_contract(&translation, &recompile_contract)
        .map_err(SettlementError::Recompile)?;
    let residual_obligations = compile_residual_obligations(&translation, &recompile_contract);

    Ok(IntentEditSettlementPlanV1 {
        schema_id: INTENT_EDIT_SETTLEMENT_PLAN_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        translation,
        recompile_contract,
        residual_obligations,
    })
}

pub fn validate_settlement_plan(
    settlement: &IntentEditSettlementPlanV1,
) -> Result<(), SettlementError> {
    if settlement.schema_id != INTENT_EDIT_SETTLEMENT_PLAN_SCHEMA_ID {
        return Err(SettlementError::InvalidSchemaId {
            observed: settlement.schema_id.clone(),
        });
    }
    validate_recompile_contract(&settlement.translation, &settlement.recompile_contract)
        .map_err(SettlementError::Recompile)?;
    let expected =
        compile_residual_obligations(&settlement.translation, &settlement.recompile_contract);
    if expected.len() != settlement.residual_obligations.len() {
        return Err(SettlementError::ResidualCountMismatch {
            expected: expected.len(),
            observed: settlement.residual_obligations.len(),
        });
    }
    Ok(())
}

fn compile_residual_obligations(
    translation: &RepoEditTranslationPlanV1,
    contract: &IntentEditRecompileContractV1,
) -> Vec<IntentEditResidualObligationV1> {
    let mut obligations = Vec::new();
    for request in &translation.requests {
        obligations.push(IntentEditResidualObligationV1 {
            obligation_id: format!("apply-receipt:{}", request.action_id),
            kind: IntentEditResidualObligationKindV1::AwaitApplyReceipt,
            action_id: Some(request.action_id.clone()),
            statement: format!("collect repo-edit apply receipt for {}", request.target),
        });
    }
    for item in &contract.obligations {
        obligations.push(IntentEditResidualObligationV1 {
            obligation_id: format!("recompile-proof:{}", item.action_id),
            kind: IntentEditResidualObligationKindV1::AwaitRecompileProof,
            action_id: Some(item.action_id.clone()),
            statement: item.statement.clone(),
        });
    }
    obligations.push(IntentEditResidualObligationV1 {
        obligation_id: format!("currentness-refresh:{}", translation.plan_id),
        kind: IntentEditResidualObligationKindV1::AwaitCurrentnessRefresh,
        action_id: None,
        statement: "refresh snapshot currentness after apply and recompile proof".to_string(),
    });
    obligations
}
