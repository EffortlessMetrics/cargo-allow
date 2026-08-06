use allow_core::{CargoAllowError, CargoAllowErrorKind};
use repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};

use super::{
    CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
    CoreCommandSummaryInputV1, CoreCommandSummaryV1, CoreCommandWritePostureV1,
    CoreSourceSubjectV1, build_core_command_summary,
};

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
        currentness: CurrentnessV1::Current,
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
        operation_effects: CoreCommandEffectsV1::read_only(plan.explicit_non_effects.clone()),
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
        allow_report::BootstrapDisposition::InvalidPolicy => (
            ResultClassV1::MalformedInput,
            CoreCommandPostureV1::Blocking,
        ),
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            (ResultClassV1::Unsupported, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::InstrumentFailure => (
            ResultClassV1::InstrumentFailure,
            CoreCommandPostureV1::Blocking,
        ),
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
        format!("adoption.{}", action.kind.as_str()),
        format!("Run {}", action.kind.as_str()),
        program.clone(),
        argv.cloned().collect(),
    )
    .with_write_posture(write_posture, write_paths)
    .with_contract(
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

fn error_result(kind: CargoAllowErrorKind) -> (ResultClassV1, CompletenessV1, CurrentnessV1) {
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
