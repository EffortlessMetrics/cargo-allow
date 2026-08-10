//! Bounded staged pre-commit command and receipt projection.
//!
//! The command owns orchestration and delegation only. Authoritative precommit
//! evaluation is provided by cargo-intent via repo.analysis-receipt.v1 (#2601/#2568).

use crate::check::CheckArgs;
use crate::precommit_tool::CargoAllowToolIdentityV1;
use crate::{
    OutputFormat, RootArgs, assert_path_within_root, current_dir, emit_text,
    resolve_source_tree_root, root_relative_path, write_file,
};
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use effortless_repo_snapshot::{
    StagedPathChange, StagedPathStatus, StagedRepositorySnapshot, StagedSnapshotCompleteness,
    staged_repository_snapshot,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const SPEC_PRECOMMIT_SCHEMA_ID: &str = "cargo-allow.spec-precommit.v1";
pub const SPEC_PRECOMMIT_SCHEMA_VERSION: u32 = 1;
const CLAIM_BOUNDARY: &str = "Exact staged source posture via cargo-intent delegation; no embedded evaluator, project execution, runtime proof, hosted CI, hook installation, or release promotion.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
enum SpecPrecommitResultClass {
    Passed,
    FindingsAdvisory,
    FindingsBlocking,
    NotApplicable,
    PartialData,
    StaleInput,
    Unsupported,
    MalformedInput,
    InstrumentFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct StagedIdentityV1 {
    schema_id: &'static str,
    schema_version: u32,
    parent_commit: Option<String>,
    staged_identity: String,
    staged_path_count: usize,
    staged_change_count: usize,
    completeness: &'static str,
    limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct StagedChangeV1 {
    status: String,
    path: Option<String>,
    previous_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ToolSelectionSummaryV1 {
    result: String,
    mode: String,
    executable_digest: Option<String>,
    channel: Option<String>,
    preview_evidence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PrecommitFindingV1 {
    code: String,
    subject: String,
    posture: String,
    message: String,
    action: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SpecPrecommitReportV1 {
    schema_id: &'static str,
    schema_version: u32,
    command: &'static str,
    phase: &'static str,
    profile: &'static str,
    tool_identity: Option<CargoAllowToolIdentityV1>,
    tool_selection: Option<ToolSelectionSummaryV1>,
    parent_commit: Option<String>,
    parent_tree: Option<String>,
    staged_identity_before: Option<String>,
    staged_identity_after: Option<String>,
    staged_changes: Vec<StagedChangeV1>,
    change_class: Option<String>,
    findings: Vec<PrecommitFindingV1>,
    result_class: SpecPrecommitResultClass,
    process_exit_family: &'static str,
    inventory_completeness: &'static str,
    source_view_identity: Option<String>,
    tool_result_class: Option<String>,
    duration_ms: u128,
    remaining_gates: Vec<&'static str>,
    error: Option<String>,
    claim_boundary: &'static str,
}

pub(crate) fn cmd_staged_identity(args: &CheckArgs) -> CargoAllowResult<()> {
    if args.phase.is_some() || args.profile.is_some() || args.expect_staged_identity.is_some() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--staged-identity-only cannot be combined with --profile, --phase, or --expect-staged-identity",
        ));
    }
    let root = resolve_root(&args.root)?;
    let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(&root))?;
    validate_output_paths(
        &root,
        &snapshot,
        args.output.as_deref(),
        args.receipt.as_deref(),
    )?;
    let identity = StagedIdentityV1 {
        schema_id: "cargo-allow.staged-snapshot.v1",
        schema_version: 1,
        parent_commit: snapshot.parent_commit.clone(),
        staged_identity: snapshot.identity.semantic_hash.clone(),
        staged_path_count: snapshot.entries.len(),
        staged_change_count: snapshot.changes.len(),
        completeness: snapshot_completeness(snapshot.completeness),
        limitations: snapshot.limitations.clone(),
    };
    emit_identity(args, &identity)
}

pub(crate) fn cmd_spec_precommit(args: &CheckArgs) -> CargoAllowResult<()> {
    let started = Instant::now();
    match crate::intent_delegate::try_delegate_staged_precommit(args, started)? {
        crate::intent_delegate::DelegationDisposition::Handle(result) => result,
        crate::intent_delegate::DelegationDisposition::Disabled => {
            let root = resolve_root(&args.root)?;
            crate::intent_delegate::reject_embedded_precommit_authority(&root)?;
            fail_precommit_without_delegation(args, &root, started)
        }
    }
}

fn fail_precommit_without_delegation(
    args: &CheckArgs,
    root: &Path,
    started: Instant,
) -> CargoAllowResult<()> {
    let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(root))?;
    finish_failure(
        args,
        &snapshot,
        None,
        None,
        FailureOutcome {
            result: SpecPrecommitResultClass::Unsupported,
            exit_family: "provider_unavailable",
            message: format!(
                "staged precommit requires cargo-intent delegation; set delegate_staged_precommit = true in {}",
                crate::intent_provider::DEFAULT_INTENT_DELEGATION_CONFIG
            ),
            duration_ms: started.elapsed().as_millis(),
        },
    )
}

struct FailureOutcome {
    result: SpecPrecommitResultClass,
    exit_family: &'static str,
    message: String,
    duration_ms: u128,
}

fn finish_failure(
    args: &CheckArgs,
    snapshot: &StagedRepositorySnapshot,
    identity: Option<CargoAllowToolIdentityV1>,
    selection: Option<ToolSelectionSummaryV1>,
    outcome: FailureOutcome,
) -> CargoAllowResult<()> {
    let report = SpecPrecommitReportV1 {
        schema_id: SPEC_PRECOMMIT_SCHEMA_ID,
        schema_version: SPEC_PRECOMMIT_SCHEMA_VERSION,
        command: "check",
        phase: "precommit",
        profile: "spec-system",
        tool_identity: identity,
        tool_selection: selection,
        parent_commit: snapshot.parent_commit.clone(),
        parent_tree: None,
        staged_identity_before: Some(snapshot.identity.semantic_hash.clone()),
        staged_identity_after: None,
        staged_changes: snapshot.changes.iter().map(staged_change).collect(),
        change_class: None,
        findings: Vec::new(),
        result_class: outcome.result,
        process_exit_family: outcome.exit_family,
        inventory_completeness: snapshot_completeness(snapshot.completeness),
        source_view_identity: None,
        tool_result_class: None,
        duration_ms: outcome.duration_ms,
        remaining_gates: vec!["evaluation did not reach objective policy"],
        error: Some(outcome.message.clone()),
        claim_boundary: CLAIM_BOUNDARY,
    };
    let root = resolve_root(&args.root)?;
    emit_report(args, &root, &report)?;
    let kind = match outcome.result {
        SpecPrecommitResultClass::MalformedInput => CargoAllowErrorKind::InvalidConfig,
        SpecPrecommitResultClass::Unsupported => CargoAllowErrorKind::InvalidConfig,
        SpecPrecommitResultClass::StaleInput => CargoAllowErrorKind::Inventory,
        SpecPrecommitResultClass::FindingsBlocking => CargoAllowErrorKind::PolicyViolation,
        _ => CargoAllowErrorKind::Internal,
    };
    Err(CargoAllowError::with_kind(kind, outcome.message))
}

fn emit_identity(args: &CheckArgs, identity: &StagedIdentityV1) -> CargoAllowResult<()> {
    let rendered = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(identity).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("failed to render staged identity JSON: {error}"),
            )
        })?,
        OutputFormat::Human | OutputFormat::Markdown => render_identity_human(identity),
        OutputFormat::Html | OutputFormat::Sarif => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "staged identity supports only human, markdown, and JSON output",
            ));
        }
    };
    let should_print =
        args.output.is_some() || args.receipt.is_none() || args.format == OutputFormat::Json;
    if should_print {
        emit_text(args.output.as_deref(), &rendered)?;
    }
    if let Some(receipt) = &args.receipt {
        write_file(
            receipt,
            &serde_json::to_string_pretty(identity).map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!("failed to render staged identity receipt: {error}"),
                )
            })?,
        )?;
    }
    Ok(())
}

fn emit_report(
    args: &CheckArgs,
    root: &Path,
    report: &SpecPrecommitReportV1,
) -> CargoAllowResult<()> {
    let json = serde_json::to_string_pretty(report).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to render staged precommit JSON: {error}"),
        )
    })?;
    let rendered = match args.format {
        OutputFormat::Json => json.clone(),
        OutputFormat::Human | OutputFormat::Markdown => render_human_report(report),
        OutputFormat::Html | OutputFormat::Sarif => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "staged precommit supports only human, markdown, and JSON output",
            ));
        }
    };
    let should_print =
        args.output.is_some() || args.receipt.is_none() || args.format == OutputFormat::Json;
    if should_print {
        emit_text(args.output.as_deref(), &rendered)?;
    }
    if let Some(receipt) = &args.receipt {
        assert_path_within_root(root, receipt)?;
        write_file(receipt, &json)?;
    }
    Ok(())
}

fn render_identity_human(identity: &StagedIdentityV1) -> String {
    format!(
        "Candidate\n  parent: {}\n  staged identity: {}\n  staged paths: {}\n  staged changes: {}\n  completeness: {}\n",
        identity.parent_commit.as_deref().unwrap_or("unborn"),
        identity.staged_identity,
        identity.staged_path_count,
        identity.staged_change_count,
        identity.completeness,
    )
}

fn render_human_report(report: &SpecPrecommitReportV1) -> String {
    let mut text = String::new();
    text.push_str("Candidate\n");
    text.push_str(&format!(
        "  parent: {}\n  staged identity before: {}\n  staged identity after: {}\n  changes: {}\n  class: {}\n",
        report.parent_commit.as_deref().unwrap_or("unborn"),
        report.staged_identity_before.as_deref().unwrap_or("unknown"),
        report.staged_identity_after.as_deref().unwrap_or("unknown"),
        report.staged_changes.len(),
        report.change_class.as_deref().unwrap_or("unknown"),
    ));
    text.push_str("\nTool\n");
    if let Some(tool) = &report.tool_selection {
        text.push_str(&format!(
            "  result: {}\n  mode: {}\n  digest: {}\n  preview evidence: {}\n",
            tool.result,
            tool.mode,
            tool.executable_digest.as_deref().unwrap_or("unknown"),
            tool.preview_evidence,
        ));
    } else {
        text.push_str("  result: unavailable\n");
    }
    text.push_str("\nFindings\n");
    if report.findings.is_empty() {
        text.push_str("  none\n");
    } else {
        for finding in &report.findings {
            text.push_str(&format!(
                "  [{}] {} ({})\n    {}\n    next: {}\n",
                finding.posture, finding.code, finding.subject, finding.message, finding.action
            ));
        }
    }
    text.push_str(&format!(
        "\nResult\n  class: {:?}\n  exit family: {}\n  inventory: {}\n  claim: {}\n",
        report.result_class,
        report.process_exit_family,
        report.inventory_completeness,
        report.claim_boundary,
    ));
    if let Some(error) = &report.error {
        text.push_str(&format!("  error: {error}\n"));
    }
    text
}

fn resolve_root(args: &RootArgs) -> CargoAllowResult<PathBuf> {
    let cwd = current_dir()?;
    resolve_source_tree_root(args.root.as_deref(), cwd)
}

fn validate_output_paths(
    root: &Path,
    snapshot: &StagedRepositorySnapshot,
    output: Option<&Path>,
    receipt: Option<&Path>,
) -> CargoAllowResult<()> {
    for path in [output, receipt].into_iter().flatten() {
        assert_path_within_root(root, path)?;
        let absolute = root_relative_path(root, path);
        let relative = absolute.strip_prefix(root).unwrap_or(absolute.as_path());
        if snapshot
            .entries
            .iter()
            .filter_map(|entry| entry.path.as_deref())
            .any(|candidate| candidate == relative)
        {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!(
                    "output path `{}` is part of the staged candidate",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn staged_change(change: &StagedPathChange) -> StagedChangeV1 {
    StagedChangeV1 {
        status: staged_status(change.status).to_string(),
        path: change.path.as_ref().map(|path| path.display().to_string()),
        previous_path: change
            .previous_path
            .as_ref()
            .map(|path| path.display().to_string()),
    }
}

fn staged_status(status: StagedPathStatus) -> &'static str {
    match status {
        StagedPathStatus::Added => "added",
        StagedPathStatus::Modified => "modified",
        StagedPathStatus::Deleted => "deleted",
        StagedPathStatus::Renamed => "renamed",
        StagedPathStatus::Copied => "copied",
        StagedPathStatus::TypeChanged => "type_changed",
        StagedPathStatus::Unmerged => "unmerged",
        StagedPathStatus::Unknown => "unknown",
    }
}

fn snapshot_completeness(completeness: StagedSnapshotCompleteness) -> &'static str {
    match completeness {
        StagedSnapshotCompleteness::Complete => "complete",
        StagedSnapshotCompleteness::Partial => "partial",
    }
}

fn static_exit_family(family: &str) -> &'static str {
    match family {
        "success" => "success",
        "blocking" => "blocking",
        "advisory" => "advisory",
        "usage" => "usage",
        _ => "instrument_failure",
    }
}

pub(crate) struct DelegatedPrecommitOutcome {
    pub result_class: effortless_repo_protocol::ResultClassV1,
    pub exit_success: bool,
    pub staged_identity: String,
    pub process_exit_family: String,
    pub provider_claim_boundary: Option<String>,
    pub unmapped_staged_surface: bool,
    pub error: Option<String>,
}

impl DelegatedPrecommitOutcome {
    pub(crate) fn from_delegate_failure(
        failure: &crate::intent_delegate::IntentDelegateFailure,
    ) -> Self {
        let result_class = match failure.class {
            crate::intent_delegate::IntentDelegateFailureClass::StaleSource => {
                effortless_repo_protocol::ResultClassV1::StaleInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::MalformedOutput => {
                effortless_repo_protocol::ResultClassV1::MalformedInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::WrongProduct
            | crate::intent_delegate::IntentDelegateFailureClass::WrongProtocol => {
                effortless_repo_protocol::ResultClassV1::MalformedInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::Timeout => {
                effortless_repo_protocol::ResultClassV1::InstrumentFailure
            }
            crate::intent_delegate::IntentDelegateFailureClass::ProviderAbsent
            | crate::intent_delegate::IntentDelegateFailureClass::IdentityMismatch
            | crate::intent_delegate::IntentDelegateFailureClass::InstrumentFailure => {
                effortless_repo_protocol::ResultClassV1::InstrumentFailure
            }
        };
        Self {
            result_class,
            exit_success: false,
            staged_identity: String::new(),
            process_exit_family: "instrument_failure".to_string(),
            provider_claim_boundary: None,
            unmapped_staged_surface: false,
            error: Some(failure.to_string()),
        }
    }
}

pub(crate) fn complete_delegated_precommit(
    args: &CheckArgs,
    root: &Path,
    snapshot: &StagedRepositorySnapshot,
    outcome: DelegatedPrecommitOutcome,
    elapsed: Duration,
) -> CargoAllowResult<()> {
    let result_class = if snapshot.changes.is_empty() && outcome.error.is_none() {
        SpecPrecommitResultClass::NotApplicable
    } else {
        map_delegated_result_class(&outcome)
    };
    let exit_family = if outcome.error.is_some() {
        "instrument_failure"
    } else {
        static_exit_family(&outcome.process_exit_family)
    };
    let remaining_gates = if outcome.unmapped_staged_surface {
        vec![
            "delegated via repo.analysis-receipt.v1",
            "provider reported unmapped staged surface",
        ]
    } else {
        vec![
            "delegated via repo.analysis-receipt.v1",
            "provider obligation skeleton only; no embedded evaluator",
        ]
    };
    let report = SpecPrecommitReportV1 {
        schema_id: SPEC_PRECOMMIT_SCHEMA_ID,
        schema_version: SPEC_PRECOMMIT_SCHEMA_VERSION,
        command: "check",
        phase: "precommit",
        profile: "spec-system",
        tool_identity: None,
        tool_selection: None,
        parent_commit: snapshot.parent_commit.clone(),
        parent_tree: None,
        staged_identity_before: Some(snapshot.identity.semantic_hash.clone()),
        staged_identity_after: Some(outcome.staged_identity.clone()),
        staged_changes: snapshot.changes.iter().map(staged_change).collect(),
        change_class: None,
        findings: Vec::new(),
        result_class,
        process_exit_family: exit_family,
        inventory_completeness: snapshot_completeness(snapshot.completeness),
        source_view_identity: None,
        tool_result_class: Some(
            outcome
                .provider_claim_boundary
                .clone()
                .unwrap_or_else(|| format!("{:?}", outcome.result_class)),
        ),
        duration_ms: elapsed.as_millis(),
        remaining_gates,
        error: outcome.error.clone(),
        claim_boundary: CLAIM_BOUNDARY,
    };
    emit_report(args, root, &report)?;
    if matches!(
        result_class,
        SpecPrecommitResultClass::Passed
            | SpecPrecommitResultClass::FindingsAdvisory
            | SpecPrecommitResultClass::NotApplicable
    ) && outcome.exit_success
    {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            outcome
                .error
                .unwrap_or_else(|| "delegated staged precommit posture is not green".to_string()),
        ))
    }
}

fn map_delegated_result_class(outcome: &DelegatedPrecommitOutcome) -> SpecPrecommitResultClass {
    if outcome.error.is_some() {
        return match outcome.result_class {
            effortless_repo_protocol::ResultClassV1::StaleInput => {
                SpecPrecommitResultClass::StaleInput
            }
            effortless_repo_protocol::ResultClassV1::MalformedInput => {
                SpecPrecommitResultClass::MalformedInput
            }
            _ => SpecPrecommitResultClass::InstrumentFailure,
        };
    }
    match outcome.result_class {
        effortless_repo_protocol::ResultClassV1::Completed => SpecPrecommitResultClass::Passed,
        effortless_repo_protocol::ResultClassV1::Findings => {
            if outcome.process_exit_family == "advisory" {
                SpecPrecommitResultClass::FindingsAdvisory
            } else {
                SpecPrecommitResultClass::FindingsBlocking
            }
        }
        effortless_repo_protocol::ResultClassV1::PartialData => {
            SpecPrecommitResultClass::PartialData
        }
        effortless_repo_protocol::ResultClassV1::StaleInput => SpecPrecommitResultClass::StaleInput,
        effortless_repo_protocol::ResultClassV1::MalformedInput => {
            SpecPrecommitResultClass::MalformedInput
        }
        effortless_repo_protocol::ResultClassV1::Unsupported => {
            SpecPrecommitResultClass::Unsupported
        }
        effortless_repo_protocol::ResultClassV1::InstrumentFailure => {
            SpecPrecommitResultClass::InstrumentFailure
        }
        effortless_repo_protocol::ResultClassV1::NotProven
        | effortless_repo_protocol::ResultClassV1::Cancelled
        | effortless_repo_protocol::ResultClassV1::Conflict => {
            SpecPrecommitResultClass::InstrumentFailure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::error::Error;
    use std::fs;

    fn output_path(label: &str) -> PathBuf {
        PathBuf::from(format!(
            "target/cargo-allow/spec-precommit-{label}-{}.json",
            std::process::id()
        ))
    }

    fn check_args(output: Option<PathBuf>, receipt: Option<PathBuf>) -> CheckArgs {
        CheckArgs {
            root: RootArgs::default(),
            config: None,
            profile: Some(crate::ProfileArg::SpecSystem),
            compat: false,
            kind: None,
            include_untracked: false,
            format: OutputFormat::Json,
            output,
            receipt,
            mode: None,
            deny: Vec::new(),
            phase: Some(crate::check::CheckPhase::Precommit),
            staged: true,
            staged_identity_only: false,
            expect_staged_identity: None,
            tool_mode: None,
            tool_digest: None,
            preview_authorized: false,
        }
    }

    #[test]
    fn spec_precommit_cli() -> Result<(), Box<dyn Error>> {
        let parsed = crate::cli::CargoAllowCli::try_parse_from([
            "cargo-allow",
            "check",
            "--profile",
            "spec-system",
            "--phase",
            "precommit",
            "--staged",
            "--format",
            "json",
        ])?;
        let Some(crate::cli::CargoAllowCommand::Check(args)) = parsed.command else {
            return Err("staged precommit CLI did not parse as check".into());
        };
        if !args.staged
            || args.phase != Some(crate::check::CheckPhase::Precommit)
            || args.profile != Some(crate::ProfileArg::SpecSystem)
        {
            return Err("staged precommit CLI lost its phase, staged, or profile contract".into());
        }
        Ok(())
    }

    #[test]
    fn spec_precommit_result_classes() -> Result<(), Box<dyn Error>> {
        let encoded = serde_json::to_string(&[
            SpecPrecommitResultClass::Passed,
            SpecPrecommitResultClass::FindingsBlocking,
            SpecPrecommitResultClass::StaleInput,
            SpecPrecommitResultClass::InstrumentFailure,
        ])?;
        for required in [
            "Passed",
            "FindingsBlocking",
            "StaleInput",
            "InstrumentFailure",
        ] {
            if !encoded.contains(required) {
                return Err(format!("result class `{required}` was not serialized").into());
            }
        }
        Ok(())
    }

    #[test]
    fn spec_precommit_receipt() -> Result<(), Box<dyn Error>> {
        let output = output_path("receipt-output");
        let receipt = output_path("receipt");
        let _ = fs::remove_file(&output);
        let _ = fs::remove_file(&receipt);
        let mut args = check_args(Some(output.clone()), Some(receipt.clone()));
        args.profile = None;
        args.phase = None;
        cmd_staged_identity(&args)?;
        let output_value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&output)?)?;
        let receipt_value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&receipt)?)?;
        let _ = fs::remove_file(&output);
        let _ = fs::remove_file(&receipt);
        if output_value != receipt_value {
            return Err("staged identity output and receipt diverged".into());
        }
        Ok(())
    }

    #[test]
    fn spec_precommit_identity_handshake() -> Result<(), Box<dyn Error>> {
        let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(
            resolve_root(&RootArgs::default())?,
        ))?;
        let output = output_path("identity-handshake");
        let _ = fs::remove_file(&output);
        let mut args = check_args(Some(output.clone()), None);
        args.expect_staged_identity = Some(snapshot.identity.semantic_hash);
        let result = cmd_spec_precommit(&args);
        let _ = fs::remove_file(&output);
        if result.is_ok() {
            return Err("precommit without delegation should fail".into());
        }
        Ok(())
    }
}
