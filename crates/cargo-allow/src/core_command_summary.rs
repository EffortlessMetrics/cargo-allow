//! Compact operator summary over existing cargo-allow command semantics.
//!
//! The summary is an additive projection. Command-specific reports, receipts,
//! diagnostics, and action plans remain authoritative for their full meaning.
//! This module does not scan source, evaluate policy, select repository
//! judgments, or execute a suggested action.

use allow_core::{CargoAllowError, CargoAllowErrorKind};
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
    pub display: String,
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
        args: impl IntoIterator<Item = impl Into<String>>,
        write_posture: CoreCommandWritePostureV1,
        may_write_paths: impl IntoIterator<Item = String>,
        reason: impl Into<String>,
        expected_effect: impl Into<String>,
        proof_boundary: impl Into<String>,
    ) -> Self {
        let program = program.into();
        let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        let display = render_argv_for_display(&program, &args);
        Self {
            id: id.into(),
            kind: CoreCommandActionKindV1::Command,
            title: title.into(),
            program: Some(program),
            args,
            display,
            write_posture,
            may_write_paths: may_write_paths.into_iter().collect(),
            reason: reason.into(),
            expected_effect: expected_effect.into(),
            proof_boundary: proof_boundary.into(),
        }
    }

    pub fn decision(
        id: impl Into<String>,
        title: impl Into<String>,
        reason: impl Into<String>,
        expected_effect: impl Into<String>,
        proof_boundary: impl Into<String>,
    ) -> Self {
        let title = title.into();
        Self {
            id: id.into(),
            kind: CoreCommandActionKindV1::Decision,
            display: title.clone(),
            title,
            program: None,
            args: Vec::new(),
            write_posture: CoreCommandWritePostureV1::ReadOnly,
            may_write_paths: Vec::new(),
            reason: reason.into(),
            expected_effect: expected_effect.into(),
            proof_boundary: proof_boundary.into(),
        }
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
    pub fn read_only(explicit_non_effects: impl IntoIterator<Item = String>) -> Self {
        Self {
            reads_repository: true,
            writes_repository: false,
            executes_repository_code: false,
            invokes_network: false,
            write_paths: Vec::new(),
            explicit_non_effects: explicit_non_effects.into_iter().collect(),
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
    require_non_empty("tool_version", &summary.tool_version)?;
    require_non_empty("operation", &summary.operation)?;
    require_non_empty("subject.repository_identity", &summary.subject.repository_identity)?;
    require_non_empty("subject.portable_identity", &summary.subject.portable_identity)?;
    require_non_empty("reason.code", &summary.reason.code)?;
    require_non_empty("reason.message", &summary.reason.message)?;
    require_non_empty("claim_boundary.statement", &summary.claim_boundary.statement)?;

    if summary.result_class == ResultClassV1::Completed {
        if summary.completeness != CompletenessV1::Complete {
            return Err("completed summary requires complete coverage".to_string());
        }
        if summary.currentness != CurrentnessV1::Current {
            return Err("completed summary requires current evidence".to_string());
        }
        if summary.posture == CoreCommandPostureV1::Blocking {
            return Err("completed summary cannot carry blocking posture".to_string());
        }
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
    if summary.additional_action_count > 0 && summary.additional_actions_ref.is_none() {
        return Err("additional actions require a retrieval reference".to_string());
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
    require_non_empty("action.display", &action.display)?;
    require_non_empty("action.reason", &action.reason)?;
    require_non_empty("action.expected_effect", &action.expected_effect)?;
    require_non_empty("action.proof_boundary", &action.proof_boundary)?;
    if action.kind == CoreCommandActionKindV1::Command
        && action
            .program
            .as_deref()
            .is_none_or(|program| program.trim().is_empty())
    {
        return Err("command action requires a program".to_string());
    }
    if matches!(
        action.write_posture,
        CoreCommandWritePostureV1::CandidateWrite | CoreCommandWritePostureV1::LiveMutation
    ) && action.may_write_paths.is_empty()
    {
        return Err("writing action must name its possible write paths".to_string());
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

pub fn core_command_summary_from_adoption_plan(
    plan: &allow_report::CoreAdoptionPlanV1,
) -> Result<CoreCommandSummaryV1, String> {
    let (result_class, posture) = adoption_result(plan.bootstrap_disposition);
    let completeness = match plan.inventory.completeness {
        allow_report::InventoryCompleteness::Complete => CompletenessV1::Complete,
        allow_report::InventoryCompleteness::Partial => CompletenessV1::Partial,
        allow_report::InventoryCompleteness::Unknown => CompletenessV1::Unknown,
    };
    let primary_action = action_from_adoption(&plan.primary_action, &plan.may_write_paths)?;
    let next_proof = plan
        .follow_up_actions
        .iter()
        .find(|action| action.kind == allow_report::AdoptionActionKind::RunNoNewCheck)
        .map(|action| action_from_adoption(action, &[]))
        .transpose()?;
    let additional_action_count = plan.follow_up_actions.len();
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: plan.tool_version.clone(),
        operation: "adopt".to_string(),
        mode: None,
        profile: None,
        subject: CoreSourceSubjectV1::worktree(
            plan.repository_identity.clone(),
            format!("{}:worktree", plan.repository_identity),
        ),
        result_class,
        posture,
        completeness,
        currentness: if result_class == ResultClassV1::StaleInput {
            CurrentnessV1::Stale
        } else {
            CurrentnessV1::Current
        },
        reason: CoreCommandReasonV1 {
            code: format!(
                "adoption.{}",
                bootstrap_disposition_code(plan.bootstrap_disposition)
            ),
            message: plan.primary_action.reason.clone(),
        },
        primary_action: Some(primary_action),
        additional_action_count,
        additional_actions_ref: (additional_action_count > 0)
            .then_some("core_adoption_plan.follow_up_actions".to_string()),
        operation_effects: CoreCommandEffectsV1::read_only(
            plan.explicit_non_effects.clone(),
        ),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(plan.claim_boundary.clone())
            .with_limitations(plan.limitations.clone()),
    })
}

fn adoption_result(
    disposition: allow_report::BootstrapDisposition,
) -> (ResultClassV1, CoreCommandPostureV1) {
    match disposition {
        allow_report::BootstrapDisposition::CleanNoPolicy
        | allow_report::BootstrapDisposition::ExistingPolicyHealthy => {
            (ResultClassV1::Completed, CoreCommandPostureV1::Satisfied)
        }
        allow_report::BootstrapDisposition::FindingsNoPolicy
        | allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings
        | allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair => {
            (ResultClassV1::Findings, CoreCommandPostureV1::Advisory)
        }
        allow_report::BootstrapDisposition::PartialInventory => {
            (ResultClassV1::PartialData, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::InvalidPolicy => {
            (ResultClassV1::MalformedInput, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            (ResultClassV1::Unsupported, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::InstrumentFailure => {
            (ResultClassV1::InstrumentFailure, CoreCommandPostureV1::Blocking)
        }
    }
}

fn bootstrap_disposition_code(disposition: allow_report::BootstrapDisposition) -> &'static str {
    match disposition {
        allow_report::BootstrapDisposition::CleanNoPolicy => "clean_no_policy",
        allow_report::BootstrapDisposition::FindingsNoPolicy => "findings_no_policy",
        allow_report::BootstrapDisposition::ExistingPolicyHealthy => "existing_policy_healthy",
        allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings => {
            "existing_policy_has_new_findings"
        }
        allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair => {
            "existing_policy_needs_repair"
        }
        allow_report::BootstrapDisposition::PartialInventory => "partial_inventory",
        allow_report::BootstrapDisposition::InvalidPolicy => "invalid_policy",
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            "unsupported_repository_state"
        }
        allow_report::BootstrapDisposition::InstrumentFailure => "instrument_failure",
    }
}

fn action_from_adoption(
    action: &allow_report::AdoptionAction,
    may_write_paths: &[String],
) -> Result<CoreCommandActionV1, String> {
    let mut argv = action.argv.iter();
    let Some(program) = argv.next() else {
        return Err("adoption action argv must include a program".to_string());
    };
    let write_posture = match action.write_posture {
        allow_report::WritePosture::ReadOnly => CoreCommandWritePostureV1::ReadOnly,
        allow_report::WritePosture::PreviewOnly => CoreCommandWritePostureV1::PreviewOnly,
        allow_report::WritePosture::MayWrite => CoreCommandWritePostureV1::LiveMutation,
    };
    let write_paths = if action.write_posture == allow_report::WritePosture::MayWrite {
        may_write_paths.to_vec()
    } else {
        Vec::new()
    };
    Ok(CoreCommandActionV1::command(
        action.kind.as_str(),
        format!("Run {}", action.kind.as_str()),
        program.clone(),
        argv.cloned(),
        write_posture,
        write_paths,
        action.reason.clone(),
        action.expected_result.clone(),
        "The action must be re-evaluated against current command-specific inputs; this summary does not execute it.",
    ))
}

pub fn core_command_summary_from_error(
    tool_version: impl Into<String>,
    operation: impl Into<String>,
    subject: CoreSourceSubjectV1,
    error: &CargoAllowError,
    operation_effects: CoreCommandEffectsV1,
    primary_action: Option<CoreCommandActionV1>,
    claim_boundary: ClaimBoundaryV1,
) -> Result<CoreCommandSummaryV1, String> {
    let (result_class, completeness, currentness) = error_result(error.kind());
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: tool_version.into(),
        operation: operation.into(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture: CoreCommandPostureV1::Blocking,
        completeness,
        currentness,
        reason: CoreCommandReasonV1 {
            code: error.code().to_string(),
            message: error.message().to_string(),
        },
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects,
        next_proof: None,
        artifacts: Vec::new(),
        claim_boundary,
    })
}

fn error_result(
    kind: CargoAllowErrorKind,
) -> (ResultClassV1, CompletenessV1, CurrentnessV1) {
    match kind {
        CargoAllowErrorKind::Usage
        | CargoAllowErrorKind::InvalidConfig
        | CargoAllowErrorKind::InvalidPolicy => (
            ResultClassV1::MalformedInput,
            CompletenessV1::Unknown,
            CurrentnessV1::NotProbed,
        ),
        CargoAllowErrorKind::PolicyViolation => (
            ResultClassV1::Findings,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
        ),
        CargoAllowErrorKind::Unsupported => (
            ResultClassV1::Unsupported,
            CompletenessV1::Unknown,
            CurrentnessV1::NotProbed,
        ),
        CargoAllowErrorKind::Inventory
        | CargoAllowErrorKind::Scan
        | CargoAllowErrorKind::Artifact
        | CargoAllowErrorKind::InstrumentFailure
        | CargoAllowErrorKind::Internal
        | CargoAllowErrorKind::Unknown => (
            ResultClassV1::InstrumentFailure,
            CompletenessV1::Unknown,
            CurrentnessV1::PartialOrUnavailable,
        ),
        _ => (
            ResultClassV1::InstrumentFailure,
            CompletenessV1::Unknown,
            CurrentnessV1::PartialOrUnavailable,
        ),
    }
}

pub fn render_core_command_summary_json(
    summary: &CoreCommandSummaryV1,
) -> Result<String, String> {
    validate_core_command_summary(summary)?;
    serde_json::to_string_pretty(summary).map_err(|error| error.to_string())
}

pub fn render_core_command_summary_human(summary: &CoreCommandSummaryV1) -> String {
    let mut output = String::new();
    output.push_str("Result: ");
    output.push_str(&result_label(summary));
    output.push('\n');
    output.push_str("Why: ");
    output.push_str(&allow_report::sanitize_terminal_text(&summary.reason.message));
    output.push('\n');
    output.push_str("Subject: ");
    output.push_str(summary.subject.kind.as_str());
    output.push(' ');
    output.push_str(&allow_report::sanitize_terminal_text(
        &summary.subject.portable_identity,
    ));
    output.push('\n');
    output.push_str("Coverage: ");
    output.push_str(summary.completeness.as_str());
    output.push_str(" / ");
    output.push_str(summary.currentness.as_str());
    if let Some(limitation) = summary.subject.limitations.first() {
        output.push_str(" — ");
        output.push_str(&allow_report::sanitize_terminal_text(limitation));
    }
    output.push('\n');
    output.push_str("Next: ");
    match summary.primary_action.as_ref() {
        Some(action) => output.push_str(&allow_report::sanitize_terminal_text(&action.display)),
        None if summary.posture == CoreCommandPostureV1::DecisionRequired => {
            output.push_str("repository decision required")
        }
        None => output.push_str("no deterministic safe action selected"),
    }
    output.push('\n');
    output.push_str("Writes: ");
    render_writes(summary, &mut output);
    output.push('\n');
    output.push_str("Then: ");
    match summary.next_proof.as_ref() {
        Some(action) => output.push_str(&allow_report::sanitize_terminal_text(&action.display)),
        None => output.push_str("no follow-up proof command selected"),
    }
    output.push('\n');
    output.push_str("Not proven: ");
    output.push_str(&allow_report::sanitize_terminal_text(
        &summary.claim_boundary.statement,
    ));
    if let Some(limitation) = summary.claim_boundary.limitations.first() {
        output.push_str(" — ");
        output.push_str(&allow_report::sanitize_terminal_text(limitation));
    }
    output.push('\n');
    output
}

fn result_label(summary: &CoreCommandSummaryV1) -> String {
    match (summary.result_class, summary.posture) {
        (ResultClassV1::Completed, CoreCommandPostureV1::Satisfied) => "satisfied".to_string(),
        (ResultClassV1::Findings, CoreCommandPostureV1::Advisory) => {
            "findings (advisory)".to_string()
        }
        (ResultClassV1::Findings, CoreCommandPostureV1::Blocking) => {
            "findings (blocking)".to_string()
        }
        _ => format!(
            "{} ({})",
            summary.result_class.as_str(),
            summary.posture.as_str()
        ),
    }
}

fn render_writes(summary: &CoreCommandSummaryV1, output: &mut String) {
    if summary.operation_effects.writes_repository {
        output.push_str(&join_paths(&summary.operation_effects.write_paths));
        return;
    }
    output.push_str("nothing in this operation");
    if let Some(action) = summary.primary_action.as_ref()
        && !action.may_write_paths.is_empty()
    {
        output.push_str("; selected next action may write ");
        output.push_str(&join_paths(&action.may_write_paths));
    }
}

fn join_paths(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| allow_report::sanitize_terminal_text(path))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_argv_for_display(program: &str, args: &[String]) -> String {
    if std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .any(|part| part.contains('\0') || part.contains('\n') || part.contains('\r'))
    {
        return "[use structured argv; command contains non-pasteable control text]".to_string();
    }
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote_for_platform)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_for_platform(argument: &str) -> String {
    if cfg!(windows) {
        quote_windows_cmd(argument)
    } else {
        quote_posix(argument)
    }
}

fn quote_posix(argument: &str) -> String {
    if argument.is_empty() {
        return "''".to_string();
    }
    if argument.bytes().all(|byte| {
        matches!(
            byte,
            b'a'..=b'z'
                | b'A'..=b'Z'
                | b'0'..=b'9'
                | b'_'
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b'@'
                | b'+'
                | b'='
                | b','
                | b'%'
        )
    }) {
        return argument.to_string();
    }
    let mut output = String::from("'");
    for character in argument.chars() {
        if character == '\'' {
            output.push_str("'\\''");
        } else {
            output.push(character);
        }
    }
    output.push('\'');
    output
}

fn quote_windows_cmd(argument: &str) -> String {
    if argument.is_empty() {
        return "\"\"".to_string();
    }
    if !argument.chars().any(|character| {
        matches!(
            character,
            ' ' | '\t'
                | '"'
                | '&'
                | '|'
                | '<'
                | '>'
                | '^'
                | '%'
                | '!'
                | '('
                | ')'
                | ','
                | ';'
                | '='
        )
    }) {
        return argument.to_string();
    }
    let mut output = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                for _ in 0..(backslashes * 2 + 1) {
                    output.push('\\');
                }
                backslashes = 0;
                output.push('"');
            }
            _ => {
                for _ in 0..backslashes {
                    output.push('\\');
                }
                backslashes = 0;
                output.push(character);
            }
        }
    }
    for _ in 0..(backslashes * 2) {
        output.push('\\');
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }

    fn base_input(operation: &str) -> CoreCommandSummaryInputV1 {
        CoreCommandSummaryInputV1 {
            tool_version: "0.2.0".to_string(),
            operation: operation.to_string(),
            mode: None,
            profile: None,
            subject: CoreSourceSubjectV1::worktree("repo:test", "repo:test:worktree"),
            result_class: ResultClassV1::Completed,
            posture: CoreCommandPostureV1::Satisfied,
            completeness: CompletenessV1::Complete,
            currentness: CurrentnessV1::Current,
            reason: CoreCommandReasonV1 {
                code: "test.complete".to_string(),
                message: "operation completed".to_string(),
            },
            primary_action: None,
            additional_action_count: 0,
            additional_actions_ref: None,
            operation_effects: CoreCommandEffectsV1::read_only([
                "does not execute repository code".to_string(),
            ]),
            next_proof: None,
            artifacts: Vec::new(),
            claim_boundary: ClaimBoundaryV1::new("source syntax only"),
        }
    }

    #[test]
    fn completed_summary_rejects_partial_coverage() -> Result<(), String> {
        let mut input = base_input("check");
        input.completeness = CompletenessV1::Partial;
        match build_core_command_summary(input) {
            Ok(_) => Err("partial coverage must not build as completed".to_string()),
            Err(error) => ensure(
                error.contains("complete coverage"),
                format!("unexpected validation error: {error}"),
            ),
        }
    }

    #[test]
    fn check_summary_keeps_human_and_json_result_semantics_equal() -> Result<(), String> {
        let mut input = base_input("check");
        input.mode = Some("no-new".to_string());
        input.result_class = ResultClassV1::Findings;
        input.posture = CoreCommandPostureV1::Blocking;
        input.reason = CoreCommandReasonV1 {
            code: "check.new_unreceipted_findings".to_string(),
            message: "one new unreceipted finding".to_string(),
        };
        input.primary_action = Some(CoreCommandActionV1::command(
            "check.inspect_finding",
            "Inspect the new finding",
            "cargo-allow",
            ["why", "--kind", "panic", "--path", "src/lib.rs", "--line", "42"],
            CoreCommandWritePostureV1::ReadOnly,
            [],
            "the blocking finding needs an exact explanation",
            "a bounded finding explanation is produced",
            "the explanation is not a policy mutation",
        ));
        let summary = build_core_command_summary(input)?;
        let human = render_core_command_summary_human(&summary);
        let json = render_core_command_summary_json(&summary)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        ensure(
            human.contains("Result: findings (blocking)"),
            format!("human result missing: {human}"),
        )?;
        ensure(
            value.pointer("/result_class").and_then(serde_json::Value::as_str)
                == Some("findings"),
            format!("JSON result mismatch: {json}"),
        )?;
        ensure(
            value.pointer("/primary_action/args/0").and_then(serde_json::Value::as_str)
                == Some("why"),
            format!("structured argv missing: {json}"),
        )
    }

    #[test]
    fn why_summary_can_route_repository_judgment_without_false_preference() -> Result<(), String> {
        let mut input = base_input("why");
        input.result_class = ResultClassV1::Findings;
        input.posture = CoreCommandPostureV1::DecisionRequired;
        input.reason = CoreCommandReasonV1 {
            code: "why.ambiguous".to_string(),
            message: "multiple allow entries compete".to_string(),
        };
        input.additional_action_count = 2;
        input.additional_actions_ref = Some("cargo-allow.why.v1.proof_plans".to_string());
        let summary = build_core_command_summary(input)?;
        let human = render_core_command_summary_human(&summary);
        ensure(
            summary.primary_action.is_none(),
            "ambiguous why must not select one candidate",
        )?;
        ensure(
            human.contains("Next: repository decision required"),
            format!("decision route missing: {human}"),
        )
    }

    #[test]
    fn mutation_summary_names_live_target_and_full_check() -> Result<(), String> {
        let mut input = base_input("add");
        input.operation_effects = CoreCommandEffectsV1 {
            reads_repository: true,
            writes_repository: true,
            executes_repository_code: false,
            invokes_network: false,
            write_paths: vec!["policy/allow.toml".to_string()],
            explicit_non_effects: vec!["does not approve the exception".to_string()],
        };
        input.reason = CoreCommandReasonV1 {
            code: "add.applied".to_string(),
            message: "one reviewed allow entry was written".to_string(),
        };
        input.next_proof = Some(CoreCommandActionV1::command(
            "add.full_check",
            "Run the full no-new check",
            "cargo-allow",
            ["check", "--mode", "no-new"],
            CoreCommandWritePostureV1::ReadOnly,
            [],
            "targeted confirmation is not full repository proof",
            "the complete current repository posture is evaluated",
            "source-syntax evaluation does not prove compiled or runtime correctness",
        ));
        let summary = build_core_command_summary(input)?;
        let human = render_core_command_summary_human(&summary);
        ensure(
            human.contains("Writes: policy/allow.toml"),
            format!("write target missing: {human}"),
        )?;
        ensure(
            human.contains("Then: cargo-allow check --mode no-new"),
            format!("full-check route missing: {human}"),
        )
    }

    #[test]
    fn adoption_adapter_reuses_typed_primary_action() -> Result<(), String> {
        let plan = allow_report::CoreAdoptionPlanV1 {
            schema_id: allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID.to_string(),
            schema_version: allow_report::CORE_ADOPTION_PLAN_SCHEMA_VERSION,
            tool_version: "0.2.0".to_string(),
            repository_identity: "repo:test".to_string(),
            selected_root: "<repository-root>".to_string(),
            channel: "candidate".to_string(),
            executable_identity: "sha256:test".to_string(),
            inventory: allow_report::AdoptionInventoryFacts {
                mode: allow_report::InventoryMode::GitTracked,
                completeness: allow_report::InventoryCompleteness::Complete,
                limitations: Vec::new(),
            },
            policy: allow_report::AdoptionPolicyFacts {
                state: allow_report::PolicyState::Absent,
                path: None,
                schema_version: None,
                digest: None,
                total_findings: 2,
                new_unreceipted_findings: 0,
                stale_entries: 0,
                location_drift_entries: 0,
                broken_evidence_entries: 0,
                review_due_entries: 0,
                expired_entries: 0,
                occurrence_headroom_entries: 0,
                mirror_divergence: false,
            },
            bootstrap_disposition: allow_report::BootstrapDisposition::FindingsNoPolicy,
            primary_action: allow_report::AdoptionAction {
                kind: allow_report::AdoptionActionKind::PreviewPropose,
                argv: vec!["cargo-allow".to_string(), "propose".to_string()],
                reason: "preview before retaining debt".to_string(),
                write_posture: allow_report::WritePosture::PreviewOnly,
                expected_result: "candidate entries are reviewable".to_string(),
            },
            follow_up_actions: vec![allow_report::AdoptionAction {
                kind: allow_report::AdoptionActionKind::RunNoNewCheck,
                argv: vec![
                    "cargo-allow".to_string(),
                    "check".to_string(),
                    "--mode".to_string(),
                    "no-new".to_string(),
                ],
                reason: "verify the selected policy".to_string(),
                write_posture: allow_report::WritePosture::ReadOnly,
                expected_result: "the full source-tree posture is evaluated".to_string(),
            }],
            may_write_paths: Vec::new(),
            explicit_non_effects: vec!["does not write policy".to_string()],
            expected_result_markers: vec!["preview".to_string()],
            ci_example_path: "docs/how-to/adopt-cargo-allow.md".to_string(),
            rollback_guide_path: "docs/how-to/rollback-cargo-allow-adoption.md".to_string(),
            limitations: vec!["source syntax only".to_string()],
            claim_boundary: "adoption recommendation only".to_string(),
        };
        let summary = core_command_summary_from_adoption_plan(&plan)?;
        ensure(
            summary.result_class == ResultClassV1::Findings,
            "findings/no-policy must remain a findings result",
        )?;
        ensure(
            summary
                .primary_action
                .as_ref()
                .and_then(|action| action.program.as_deref())
                == Some("cargo-allow"),
            "adoption primary action must keep structured program identity",
        )?;
        ensure(
            summary.next_proof.as_ref().is_some_and(|action| {
                action.args == ["check", "--mode", "no-new"]
            }),
            "adoption follow-up should expose the full check",
        )
    }

    #[test]
    fn error_adapter_maps_typed_kind_without_parsing_message_text() -> Result<(), String> {
        let error = CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            "policy contains an unknown field",
        );
        let summary = core_command_summary_from_error(
            "0.2.0",
            "check",
            CoreSourceSubjectV1::worktree("repo:test", "repo:test:worktree"),
            &error,
            CoreCommandEffectsV1::read_only(Vec::<String>::new()),
            None,
            ClaimBoundaryV1::new("source syntax only"),
        )?;
        ensure(
            summary.result_class == ResultClassV1::MalformedInput,
            "invalid policy should map to malformed input",
        )?;
        ensure(
            summary.reason.code == "E0003_INVALID_POLICY",
            format!("typed code was not preserved: {}", summary.reason.code),
        )
    }

    #[test]
    fn human_renderer_sanitizes_repository_control_text() -> Result<(), String> {
        let mut input = base_input("check");
        input.subject.portable_identity = "repo:test\nforged".to_string();
        input.reason.message = "bad\u{1b}[31m reason".to_string();
        let summary = build_core_command_summary(input)?;
        let human = render_core_command_summary_human(&summary);
        ensure(
            !human.contains("repo:test\nforged"),
            format!("subject injected a new line: {human}"),
        )?;
        ensure(
            !human.contains('\u{1b}'),
            format!("terminal escape survived rendering: {human}"),
        )
    }
}
