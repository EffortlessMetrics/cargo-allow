//! Active goal manifest DTOs (#2584-B).

use serde::Deserialize;

use super::doc_artifacts::ArtifactStatus;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveGoalManifest {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub status: ActiveGoalStatus,
    pub owner: String,
    pub created: String,
    pub objective: Option<String>,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_plan_status: Option<ArtifactStatus>,
    pub claim_boundary: Option<String>,
    #[serde(default)]
    pub work_item: Vec<ActiveGoalWorkItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveGoalStatus {
    Active,
    Done,
    Blocked,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveGoalWorkItem {
    pub id: String,
    pub status: ActiveGoalWorkItemStatus,
    pub title: String,
    pub owner: Option<String>,
    pub agent: Option<String>,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_closeout: Option<String>,
    pub closeout: Option<String>,
    #[serde(default)]
    pub proof_commands: Vec<String>,
    #[serde(default)]
    pub evidence_notes: Vec<String>,
    pub blocker_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveGoalWorkItemStatus {
    Ready,
    InProgress,
    Done,
    Blocked,
}
