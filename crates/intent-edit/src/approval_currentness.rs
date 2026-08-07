//! Approval and currentness envelopes for intent edit settlement (#2613-C).
//!
//! These DTOs bind edit authority to snapshot currentness. They do not apply
//! edits, invoke repo-edit, or run proof commands.

use effortless_repo_protocol::CurrentnessV1;
use serde::{Deserialize, Serialize};

pub const INTENT_EDIT_APPROVAL_CURRENTNESS_SCHEMA_ID: &str = "intent.edit-approval-currentness.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEditApprovalStateV1 {
    Pending,
    Approved,
    Rejected,
}

impl IntentEditApprovalStateV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEditApprovalCurrentnessV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub approval_state: IntentEditApprovalStateV1,
    pub currentness: CurrentnessV1,
    pub content_identity: String,
}

impl IntentEditApprovalCurrentnessV1 {
    pub fn new(
        plan_id: impl Into<String>,
        approval_state: IntentEditApprovalStateV1,
        currentness: CurrentnessV1,
        content_identity: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: INTENT_EDIT_APPROVAL_CURRENTNESS_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            approval_state,
            currentness,
            content_identity: content_identity.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalCurrentnessError {
    InvalidSchemaId { observed: String },
    NotApproved,
    StaleCurrentness,
    Rejected,
}

impl ApprovalCurrentnessError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::NotApproved => "not_approved",
            Self::StaleCurrentness => "stale_currentness",
            Self::Rejected => "rejected",
        }
    }
}

pub fn validate_approval_currentness(
    envelope: &IntentEditApprovalCurrentnessV1,
) -> Result<(), ApprovalCurrentnessError> {
    if envelope.schema_id != INTENT_EDIT_APPROVAL_CURRENTNESS_SCHEMA_ID {
        return Err(ApprovalCurrentnessError::InvalidSchemaId {
            observed: envelope.schema_id.clone(),
        });
    }
    match envelope.approval_state {
        IntentEditApprovalStateV1::Rejected => return Err(ApprovalCurrentnessError::Rejected),
        IntentEditApprovalStateV1::Pending => return Err(ApprovalCurrentnessError::NotApproved),
        IntentEditApprovalStateV1::Approved => {}
    }
    if envelope.currentness == CurrentnessV1::Stale {
        return Err(ApprovalCurrentnessError::StaleCurrentness);
    }
    if envelope.content_identity.trim().is_empty() {
        return Err(ApprovalCurrentnessError::StaleCurrentness);
    }
    Ok(())
}
