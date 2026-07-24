//! Intent edit plan transport and stable action identity (#2613-B).
//!
//! Plans describe intended mutations only. They do not touch the filesystem,
//! invoke repo-edit apply, compile graphs, or run proof commands.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const INTENT_EDIT_PLAN_SCHEMA_ID: &str = "intent.edit-plan.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEditActionKindV1 {
    ReplaceFile,
    CreateFile,
    DeleteFile,
}

impl IntentEditActionKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReplaceFile => "replace_file",
            Self::CreateFile => "create_file",
            Self::DeleteFile => "delete_file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "strategy")]
pub enum IntentEditTargetResolutionV1 {
    FindExisting {
        selector: String,
    },
    CreateIfMissing {
        selector: String,
        relative_path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEditActionV1 {
    pub action_id: String,
    pub kind: IntentEditActionKindV1,
    pub resolution: IntentEditTargetResolutionV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEditPlanV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub actions: Vec<IntentEditActionV1>,
}

impl IntentEditPlanV1 {
    pub fn new(plan_id: impl Into<String>, actions: Vec<IntentEditActionV1>) -> Self {
        Self {
            schema_id: INTENT_EDIT_PLAN_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentEditPlanError {
    DuplicateActionId { action_id: String },
    EmptySelector,
    MissingFindBeforeCreate { selector: String },
    InvalidSchemaId { observed: String },
}

impl IntentEditPlanError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DuplicateActionId { .. } => "duplicate_action_id",
            Self::EmptySelector => "empty_selector",
            Self::MissingFindBeforeCreate { .. } => "missing_find_before_create",
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
        }
    }
}

pub fn stable_action_id(
    kind: IntentEditActionKindV1,
    selector: &str,
) -> Result<String, IntentEditPlanError> {
    let normalized = normalize_selector(selector)?;
    Ok(format!("intent-edit:{}:{normalized}", kind.as_str()))
}

pub fn validate_edit_plan(plan: &IntentEditPlanV1) -> Result<(), IntentEditPlanError> {
    if plan.schema_id != INTENT_EDIT_PLAN_SCHEMA_ID {
        return Err(IntentEditPlanError::InvalidSchemaId {
            observed: plan.schema_id.clone(),
        });
    }
    let mut seen = BTreeSet::new();
    for action in &plan.actions {
        if !seen.insert(action.action_id.clone()) {
            return Err(IntentEditPlanError::DuplicateActionId {
                action_id: action.action_id.clone(),
            });
        }
        let selector = action_selector(&action.resolution)?;
        normalize_selector(selector)?;
        if matches!(
            action.resolution,
            IntentEditTargetResolutionV1::CreateIfMissing { .. }
        ) && !plan_has_find_for_selector(&plan.actions, selector)
        {
            return Err(IntentEditPlanError::MissingFindBeforeCreate {
                selector: selector.to_string(),
            });
        }
    }
    Ok(())
}

fn action_selector(resolution: &IntentEditTargetResolutionV1) -> Result<&str, IntentEditPlanError> {
    match resolution {
        IntentEditTargetResolutionV1::FindExisting { selector }
        | IntentEditTargetResolutionV1::CreateIfMissing { selector, .. } => Ok(selector.as_str()),
    }
}

fn normalize_selector(selector: &str) -> Result<&str, IntentEditPlanError> {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return Err(IntentEditPlanError::EmptySelector);
    }
    Ok(trimmed)
}

fn plan_has_find_for_selector(actions: &[IntentEditActionV1], selector: &str) -> bool {
    actions.iter().any(|action| {
        matches!(
            action.resolution,
            IntentEditTargetResolutionV1::FindExisting { .. }
        ) && action_selector(&action.resolution)
            .ok()
            .is_some_and(|candidate| candidate == selector)
    })
}
