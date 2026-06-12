use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;

use super::SpecSystemMode;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocArtifactLedger {
    pub schema_version: String,
    pub policy: String,
    pub owner: String,
    pub status: SpecSystemMode,
    #[serde(default)]
    pub artifact: Vec<DocArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocArtifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub status: ArtifactStatus,
    pub owner: String,
    pub created: String,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_adr: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_goal: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_closeout: Option<String>,
    pub linked_plan_status: Option<String>,
    pub standalone_reason: Option<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
    pub replaces: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Proposal,
    Spec,
    Adr,
    ImplementationPlan,
    PlanItem,
    ActiveGoal,
    SupportTier,
    PolicyLedger,
    Closeout,
    ReleaseRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Draft,
    Proposed,
    Accepted,
    Active,
    Done,
    Superseded,
}

pub fn parse_doc_artifact_ledger(input: &str) -> CargoAllowResult<DocArtifactLedger> {
    toml::from_str::<DocArtifactLedger>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse doc artifact ledger TOML: {e}")))
}
