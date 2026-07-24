//! Change obligation plan transport for proof-engine planning (#2589-A).

use serde::{Deserialize, Serialize};

pub const CHANGE_OBLIGATION_PLAN_SCHEMA_ID: &str = "proof.change-obligation-plan.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeObligationV1 {
    pub obligation_id: String,
    pub provider_id: String,
    pub proof_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeObligationPlanV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub obligations: Vec<ChangeObligationV1>,
}

impl ChangeObligationPlanV1 {
    pub fn new(plan_id: impl Into<String>, obligations: Vec<ChangeObligationV1>) -> Self {
        Self {
            schema_id: CHANGE_OBLIGATION_PLAN_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            obligations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationPlanError {
    InvalidSchemaId { observed: String },
    EmptyObligations,
    EmptyObligationId { index: usize },
    EmptyProviderId { index: usize },
}

impl ObligationPlanError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyObligations => "empty_obligations",
            Self::EmptyObligationId { .. } => "empty_obligation_id",
            Self::EmptyProviderId { .. } => "empty_provider_id",
        }
    }
}

pub fn validate_obligation_plan(plan: &ChangeObligationPlanV1) -> Result<(), ObligationPlanError> {
    if plan.schema_id != CHANGE_OBLIGATION_PLAN_SCHEMA_ID {
        return Err(ObligationPlanError::InvalidSchemaId {
            observed: plan.schema_id.clone(),
        });
    }
    if plan.obligations.is_empty() {
        return Err(ObligationPlanError::EmptyObligations);
    }
    for (index, obligation) in plan.obligations.iter().enumerate() {
        if obligation.obligation_id.trim().is_empty() {
            return Err(ObligationPlanError::EmptyObligationId { index });
        }
        if obligation.provider_id.trim().is_empty() {
            return Err(ObligationPlanError::EmptyProviderId { index });
        }
    }
    Ok(())
}

pub fn load_obligation_plan_toml(text: &str) -> Result<ChangeObligationPlanV1, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}
