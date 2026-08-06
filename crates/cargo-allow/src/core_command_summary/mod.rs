//! Compact operator summary over existing cargo-allow command semantics.
//!
//! The summary is an additive projection. Command-specific reports, receipts,
//! diagnostics, and action plans remain authoritative for their full meaning.
//! This module does not scan source, evaluate policy, select repository
//! judgments, or execute a suggested action.

mod adapters;
mod render;

pub use adapters::{core_command_summary_from_adoption_plan, core_command_summary_from_error};
pub use render::{render_core_command_summary_human, render_core_command_summary_json};

use repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};
use serde::{Deserialize, Serialize};

pub const CORE_COMMAND_SUMMARY_SCHEMA_ID: &str = "cargo-allow.core-command-summary.v1";
pub const CORE_COMMAND_SUMMARY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreCommandPostureV1 {
    Satisfied,
    Advisory,
    Blocking,
    DecisionRequired,
    NotApplicable,
}

impl CoreCommandPostureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Advisory => "advisory",
            Self::Blocking => "blocking",
            Self::DecisionRequired => "decision_required",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreSourceSubjectKindV1 {
    Worktree,
    Index,
    CommitTree,
    CommittedRange,
    ScopedPath,
    Unknown,
}

impl CoreSourceSubjectKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::Index => "index",
            Self::CommitTree => "commit_tree",
            Self::CommittedRange => "committed_range",
            Self::ScopedPath => "scoped_path",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreSourceSubjectV1 {
    pub kind: CoreSourceSubjectKindV1,
    pub repository_identity: String,
    pub portable_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl CoreSourceSubjectV1 {
    pub fn worktree(
        repository_identity: impl Into<String>,
        portable_identity: impl Into<String>,
    ) -> Self {
        Self {
            kind: CoreSourceSubjectKindV1::Worktree,
            repository_identity: repository_identity.into(),
            portable_identity: portable_identity.into(),
            base: None,
            head: None,
            paths: Vec::new(),
            limitations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreCommandActionKindV1 {
    Command,
    Navigation,
    Decision,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreCommandWritePostureV1 {
    ReadOnly,
    PreviewOnly,
    CandidateWrite,
    LiveMutation,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCommandActionV1 {
    pub id: String,
    pub kind: CoreCommandActionKindV1,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub write_posture: CoreCommandWritePostureV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub may_write_paths: Vec<String>,
    pub reason: String,
    pub expected_effect: String,
    pub proof_boundary: String,
}

impl CoreCommandActionV1 {
    pub fn command(
        id: impl Into<String>,
        title: impl Into<String>,
        program: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: CoreCommandActionKindV1::Command,
            title: title.into(),
            program: Some(program.into()),
            args,
            write_posture: CoreCommandWritePostureV1::ReadOnly,
            may_write_paths: Vec::new(),
            reason: String::new(),
            expected_effect: String::new(),
            proof_boundary: String::new(),
        }
    }

    pub fn decision(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: CoreCommandActionKindV1::Decision,
            title: title.into(),
            program: None,
            args: Vec::new(),
            write_posture: CoreCommandWritePostureV1::ReadOnly,
            may_write_paths: Vec::new(),
            reason: String::new(),
            expected_effect: String::new(),
            proof_boundary: String::new(),
        }
    }

    pub fn with_write_posture(
        mut self,
        write_posture: CoreCommandWritePostureV1,
        may_write_paths: Vec<String>,
    ) -> Self {
        self.write_posture = write_posture;
        self.may_write_paths = may_write_paths;
        self
    }

    pub fn with_contract(
        mut self,
        reason: impl Into<String>,
        expected_effect: impl Into<String>,
        proof_boundary: impl Into<String>,
    ) -> Self {
        self.reason = reason.into();
        self.expected_effect = expected_effect.into();
        self.proof_boundary = proof_boundary.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCommandEffectsV1 {
    pub reads_repository: bool,
    pub writes_repository: bool,
    pub executes_repository_code: bool,
    pub invokes_network: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explicit_non_effects: Vec<String>,
}

impl CoreCommandEffectsV1 {
    pub fn read_only(explicit_non_effects: Vec<String>) -> Self {
        Self {
            reads_repository: true,
            writes_repository: false,
            executes_repository_code: false,
            invokes_network: false,
            write_paths: Vec::new(),
            explicit_non_effects,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCommandReasonV1 {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCommandArtifactRefV1 {
    pub kind: String,
    pub schema_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreCommandSummaryInputV1 {
    pub tool_version: String,
    pub operation: String,
    pub mode: Option<String>,
    pub profile: Option<String>,
    pub subject: CoreSourceSubjectV1,
    pub result_class: ResultClassV1,
    pub posture: CoreCommandPostureV1,
    pub completeness: CompletenessV1,
    pub currentness: CurrentnessV1,
    pub reason: CoreCommandReasonV1,
    pub primary_action: Option<CoreCommandActionV1>,
    pub additional_action_count: usize,
    pub additional_actions_ref: Option<String>,
    pub operation_effects: CoreCommandEffectsV1,
    pub next_proof: Option<CoreCommandActionV1>,
    pub artifacts: Vec<CoreCommandArtifactRefV1>,
    pub claim_boundary: ClaimBoundaryV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCommandSummaryV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub tool: String,
    pub tool_version: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub subject: CoreSourceSubjectV1,
    pub result_class: ResultClassV1,
    pub posture: CoreCommandPostureV1,
    pub completeness: CompletenessV1,
    pub currentness: CurrentnessV1,
    pub reason: CoreCommandReasonV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<CoreCommandActionV1>,
    pub additional_action_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_actions_ref: Option<String>,
    pub operation_effects: CoreCommandEffectsV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_proof: Option<CoreCommandActionV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<CoreCommandArtifactRefV1>,
    pub claim_boundary: ClaimBoundaryV1,
}

pub fn build_core_command_summary(
    input: CoreCommandSummaryInputV1,
) -> Result<CoreCommandSummaryV1, String> {
    let summary = CoreCommandSummaryV1 {
        schema_id: CORE_COMMAND_SUMMARY_SCHEMA_ID.to_string(),
        schema_version: CORE_COMMAND_SUMMARY_SCHEMA_VERSION,
        tool: "cargo-allow".to_string(),
        tool_version: input.tool_version,
        operation: input.operation,
        mode: input.mode,
        profile: input.profile,
        subject: input.subject,
        result_class: input.result_class,
        posture: input.posture,
        completeness: input.completeness,
        currentness: input.currentness,
        reason: input.reason,
        primary_action: input.primary_action,
        additional_action_count: input.additional_action_count,
        additional_actions_ref: input.additional_actions_ref,
        operation_effects: input.operation_effects,
        next_proof: input.next_proof,
        artifacts: input.artifacts,
        claim_boundary: input.claim_boundary,
    };
    validate_core_command_summary(&summary)?;
    Ok(summary)
}

pub fn validate_core_command_summary(summary: &CoreCommandSummaryV1) -> Result<(), String> {
    if summary.schema_id != CORE_COMMAND_SUMMARY_SCHEMA_ID {
        return Err("unsupported core-command-summary schema ID".to_string());
    }
    if summary.schema_version != CORE_COMMAND_SUMMARY_SCHEMA_VERSION {
        return Err("unsupported core-command-summary schema version".to_string());
    }
    if summary.tool != "cargo-allow" {
        return Err("core-command-summary tool must be cargo-allow".to_string());
    }
    require_non_empty("tool_version", &summary.tool_version)?;
    require_non_empty("operation", &summary.operation)?;
    require_non_empty(
        "subject.repository_identity",
        &summary.subject.repository_identity,
    )?;
    require_non_empty(
        "subject.portable_identity",
        &summary.subject.portable_identity,
    )?;
    require_non_empty("reason.code", &summary.reason.code)?;
    require_non_empty("reason.message", &summary.reason.message)?;
    require_non_empty(
        "claim_boundary.statement",
        &summary.claim_boundary.statement,
    )?;

    if summary.result_class == ResultClassV1::Completed {
        if summary.completeness != CompletenessV1::Complete {
            return Err("completed summary requires complete coverage".to_string());
        }
        if summary.currentness != CurrentnessV1::Current {
            return Err("completed summary requires current evidence".to_string());
        }
        if matches!(
            summary.posture,
            CoreCommandPostureV1::Blocking | CoreCommandPostureV1::DecisionRequired
        ) {
            return Err("completed summary cannot carry an unresolved posture".to_string());
        }
    }
    if summary.result_class == ResultClassV1::Findings
        && summary.posture == CoreCommandPostureV1::Satisfied
    {
        return Err("findings summary cannot carry satisfied posture".to_string());
    }
    if summary.result_class == ResultClassV1::PartialData
        && summary.completeness == CompletenessV1::Complete
    {
        return Err("partial-data summary cannot claim complete coverage".to_string());
    }
    if summary.result_class == ResultClassV1::StaleInput
        && summary.currentness != CurrentnessV1::Stale
    {
        return Err("stale-input summary requires stale currentness".to_string());
    }
    match (
        summary.additional_action_count,
        summary.additional_actions_ref.as_ref(),
    ) {
        (0, Some(_)) => {
            return Err("zero additional actions cannot carry a retrieval reference".to_string());
        }
        (count, None) if count > 0 => {
            return Err("additional actions require a retrieval reference".to_string());
        }
        _ => {}
    }
    if !summary.operation_effects.writes_repository
        && !summary.operation_effects.write_paths.is_empty()
    {
        return Err("read-only operation cannot report written paths".to_string());
    }
    if summary.operation_effects.writes_repository
        && summary.operation_effects.write_paths.is_empty()
    {
        return Err("writing operation must name its write paths".to_string());
    }
    if let Some(action) = summary.primary_action.as_ref() {
        validate_action(action)?;
    }
    if let Some(action) = summary.next_proof.as_ref() {
        validate_action(action)?;
    }
    Ok(())
}

fn validate_action(action: &CoreCommandActionV1) -> Result<(), String> {
    require_non_empty("action.id", &action.id)?;
    require_non_empty("action.title", &action.title)?;
    require_non_empty("action.reason", &action.reason)?;
    require_non_empty("action.expected_effect", &action.expected_effect)?;
    require_non_empty("action.proof_boundary", &action.proof_boundary)?;

    match action.kind {
        CoreCommandActionKindV1::Command => {
            if action
                .program
                .as_deref()
                .is_none_or(|program| program.trim().is_empty())
            {
                return Err("command action requires a program".to_string());
            }
        }
        _ if action.program.is_some() || !action.args.is_empty() => {
            return Err("non-command action cannot carry command argv".to_string());
        }
        _ => {}
    }

    if matches!(
        action.write_posture,
        CoreCommandWritePostureV1::CandidateWrite | CoreCommandWritePostureV1::LiveMutation
    ) && action.may_write_paths.is_empty()
    {
        return Err("writing action must name its possible write paths".to_string());
    }
    if matches!(
        action.write_posture,
        CoreCommandWritePostureV1::ReadOnly | CoreCommandWritePostureV1::PreviewOnly
    ) && !action.may_write_paths.is_empty()
    {
        return Err("non-writing action cannot claim possible write paths".to_string());
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
