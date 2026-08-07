//! Translate validated intent edit plans into repo-edit apply requests (#2613-D).
//!
//! Translation produces portable request DTOs only. It does not invoke
//! `repo-edit::apply_single_target`, touch the filesystem, or run proof commands.

use effortless_repo_edit::SingleTargetApplyMode;

use crate::approval_currentness::{
    ApprovalCurrentnessError, IntentEditApprovalCurrentnessV1, validate_approval_currentness,
};
use crate::dialect_adapter::{DialectAdapterError, IntentEditDialectV1, adapt_selector};
use crate::edit_plan::{
    IntentEditActionKindV1, IntentEditActionV1, IntentEditPlanError, IntentEditPlanV1,
    IntentEditTargetResolutionV1, validate_edit_plan,
};

pub const INTENT_EDIT_REPO_EDIT_TRANSLATION_SCHEMA_ID: &str =
    "intent.edit-repo-edit-translation.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoEditTranslationRequestV1 {
    pub action_id: String,
    pub target: String,
    pub mode: SingleTargetApplyMode,
    pub caller_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoEditTranslationPlanV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub dialect: IntentEditDialectV1,
    pub requests: Vec<RepoEditTranslationRequestV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoEditTranslationError {
    EditPlan(IntentEditPlanError),
    Approval(ApprovalCurrentnessError),
    Dialect(DialectAdapterError),
    UnsupportedActionKind { kind: IntentEditActionKindV1 },
    PlanIdMismatch { expected: String, observed: String },
}

impl RepoEditTranslationError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EditPlan(_) => "edit_plan_invalid",
            Self::Approval(_) => "approval_invalid",
            Self::Dialect(_) => "dialect_adapter_failed",
            Self::UnsupportedActionKind { .. } => "unsupported_action_kind",
            Self::PlanIdMismatch { .. } => "plan_id_mismatch",
        }
    }
}

pub fn translate_plan_to_repo_edit(
    plan: &IntentEditPlanV1,
    approval: &IntentEditApprovalCurrentnessV1,
    dialect: IntentEditDialectV1,
) -> Result<RepoEditTranslationPlanV1, RepoEditTranslationError> {
    validate_edit_plan(plan).map_err(RepoEditTranslationError::EditPlan)?;
    validate_approval_currentness(approval).map_err(RepoEditTranslationError::Approval)?;
    if plan.plan_id != approval.plan_id {
        return Err(RepoEditTranslationError::PlanIdMismatch {
            expected: plan.plan_id.clone(),
            observed: approval.plan_id.clone(),
        });
    }

    let mut requests = Vec::with_capacity(plan.actions.len());
    for action in &plan.actions {
        requests.push(translate_action(action, dialect)?);
    }

    Ok(RepoEditTranslationPlanV1 {
        schema_id: INTENT_EDIT_REPO_EDIT_TRANSLATION_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        dialect,
        requests,
    })
}

fn translate_action(
    action: &IntentEditActionV1,
    dialect: IntentEditDialectV1,
) -> Result<RepoEditTranslationRequestV1, RepoEditTranslationError> {
    let selector = match &action.resolution {
        IntentEditTargetResolutionV1::FindExisting { selector }
        | IntentEditTargetResolutionV1::CreateIfMissing { selector, .. } => selector.as_str(),
    };
    let target = adapt_selector(dialect, selector).map_err(RepoEditTranslationError::Dialect)?;
    let mode = apply_mode_for_action(action)?;
    Ok(RepoEditTranslationRequestV1 {
        action_id: action.action_id.clone(),
        target,
        mode,
        caller_reference: action.action_id.clone(),
    })
}

fn apply_mode_for_action(
    action: &IntentEditActionV1,
) -> Result<SingleTargetApplyMode, RepoEditTranslationError> {
    match action.kind {
        IntentEditActionKindV1::ReplaceFile => Ok(SingleTargetApplyMode::AtomicReplace),
        IntentEditActionKindV1::CreateFile => Ok(SingleTargetApplyMode::CreateNewOnly),
        IntentEditActionKindV1::DeleteFile => {
            Err(RepoEditTranslationError::UnsupportedActionKind { kind: action.kind })
        }
    }
}
