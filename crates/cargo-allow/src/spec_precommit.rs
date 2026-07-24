//! Bounded staged pre-commit command and receipt projection.
//!
//! The command owns orchestration only: Git/source identity capture, tool
//! identity capture, the paired source compiler, and the pure policy evaluator
//! remain separate seams. Human output and the machine receipt are projections
//! of the same report value.

use crate::check::CheckArgs;
use crate::precommit_tool::{
    CargoAllowToolIdentityV1, ToolCompatibilityRequirement, ToolResultClass, ToolSelectionMode,
    ToolSelectionReceiptV1, ToolSelectionRequest, current_tool_identity, select_tool,
    verify_tool_unchanged,
};
use crate::{
    OutputFormat, RootArgs, assert_path_within_root, emit_text, resolve_source_tree_root,
    root_relative_path, write_file,
};
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_diff::{
    StagedPathChange, StagedPathStatus, StagedRepositorySnapshot, StagedSnapshotCompleteness,
    staged_repository_snapshot,
};
use allow_inventory::InventoryCompleteness;
use allow_policy::spec_system::{PrecommitFindingPosture, PrecommitObjectiveEvaluation};
use allow_rust::RustTestInventoryStatus;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const SPEC_PRECOMMIT_SCHEMA_ID: &str = "cargo-allow.spec-precommit.v1";
pub const SPEC_PRECOMMIT_SCHEMA_VERSION: u32 = 1;
const CLAIM_BOUNDARY: &str = "Exact staged source posture and bounded objective evidence; no project execution, runtime proof, hosted CI, hook installation, or release promotion.";

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
    let snapshot = staged_repository_snapshot(&root)?;
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
        crate::intent_delegate::DelegationDisposition::Disabled => {}
        crate::intent_delegate::DelegationDisposition::Handle(result) => return result,
    }
    let root = resolve_root(&args.root)?;
    let snapshot_before = staged_repository_snapshot(&root)?;
    validate_output_paths(
        &root,
        &snapshot_before,
        args.output.as_deref(),
        args.receipt.as_deref(),
    )?;

    let identity = match current_tool_identity() {
        Ok(identity) => identity,
        Err(error) => {
            return finish_failure(
                args,
                &snapshot_before,
                None,
                None,
                FailureOutcome {
                    result: SpecPrecommitResultClass::InstrumentFailure,
                    exit_family: "instrument_failure",
                    message: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                },
            );
        }
    };
    let mode = args
        .tool_mode
        .unwrap_or(ToolSelectionMode::ExplicitToolUnderTest);
    let preview_authorized = args.preview_authorized || args.tool_mode.is_none();
    let expected_digest = args
        .tool_digest
        .clone()
        .unwrap_or_else(|| identity.executable_digest.clone());
    let selection_request = ToolSelectionRequest {
        mode,
        executable: current_executable()?,
        expected_digest: Some(expected_digest),
        expected_build_source_commit: None,
        preview_authorized,
    };
    let selection = match select_tool(
        &selection_request,
        identity.clone(),
        &ToolCompatibilityRequirement::current(),
    ) {
        Ok(selection) => selection,
        Err(error) => {
            let result = tool_failure_result(&error.result);
            return finish_failure(
                args,
                &snapshot_before,
                Some(identity),
                Some(tool_summary_from_failure(mode, &error.result)),
                FailureOutcome {
                    result,
                    exit_family: "instrument_failure",
                    message: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                },
            );
        }
    };
    let tool_summary = Some(tool_summary(&selection));

    if let Some(expected) = args.expect_staged_identity.as_deref()
        && expected != snapshot_before.identity.semantic_hash
    {
        return finish_failure(
            args,
            &snapshot_before,
            Some(identity),
            tool_summary,
            FailureOutcome {
                result: SpecPrecommitResultClass::StaleInput,
                exit_family: "stale_input",
                message: "the staged identity did not match --expect-staged-identity".to_string(),
                duration_ms: started.elapsed().as_millis(),
            },
        );
    }

    if snapshot_before.changes.is_empty() {
        if let Err(error) =
            verify_tool_unchanged(&selection_request.executable, &selection.executable_digest)
        {
            return finish_failure(
                args,
                &snapshot_before,
                Some(identity),
                tool_summary,
                FailureOutcome {
                    result: SpecPrecommitResultClass::StaleInput,
                    exit_family: "stale_input",
                    message: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                },
            );
        }
        let report = SpecPrecommitReportV1 {
            schema_id: SPEC_PRECOMMIT_SCHEMA_ID,
            schema_version: SPEC_PRECOMMIT_SCHEMA_VERSION,
            command: "check",
            phase: "precommit",
            profile: "spec-system",
            tool_identity: Some(identity),
            tool_selection: tool_summary,
            parent_commit: snapshot_before.parent_commit.clone(),
            parent_tree: None,
            staged_identity_before: Some(snapshot_before.identity.semantic_hash.clone()),
            staged_identity_after: Some(snapshot_before.identity.semantic_hash.clone()),
            staged_changes: Vec::new(),
            change_class: None,
            findings: Vec::new(),
            result_class: SpecPrecommitResultClass::NotApplicable,
            process_exit_family: "success",
            inventory_completeness: snapshot_completeness(snapshot_before.completeness),
            source_view_identity: None,
            tool_result_class: Some(format!("{:?}", selection.result)),
            duration_ms: started.elapsed().as_millis(),
            remaining_gates: vec!["no staged change was available for objective evaluation"],
            error: None,
            claim_boundary: CLAIM_BOUNDARY,
        };
        emit_report(args, &root, &report)?;
        return Ok(());
    }

    if !staged_graph_inputs_changed(&snapshot_before) {
        if let Err(error) =
            verify_tool_unchanged(&selection_request.executable, &selection.executable_digest)
        {
            return finish_failure(
                args,
                &snapshot_before,
                Some(identity),
                tool_summary,
                FailureOutcome {
                    result: SpecPrecommitResultClass::StaleInput,
                    exit_family: "stale_input",
                    message: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                },
            );
        }
        let report = report_for_unmapped_staged_surface(
            &snapshot_before,
            identity,
            tool_summary,
            started.elapsed().as_millis(),
        );
        emit_report(args, &root, &report)?;
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            "staged source changed outside the spec-system graph inputs",
        ));
    }

    let paired = match crate::spec_system_workspace::compile_paired_self_hosted_graph(&root) {
        Ok(paired) => paired,
        Err(error) => {
            let result = error_result_class(&error);
            let exit_family = exit_family_for(result);
            return finish_failure(
                args,
                &snapshot_before,
                Some(identity),
                tool_summary,
                FailureOutcome {
                    result,
                    exit_family,
                    message: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                },
            );
        }
    };

    if let Err(error) =
        verify_tool_unchanged(&selection_request.executable, &selection.executable_digest)
    {
        return finish_failure(
            args,
            &snapshot_before,
            Some(identity),
            tool_summary,
            FailureOutcome {
                result: SpecPrecommitResultClass::StaleInput,
                exit_family: "stale_input",
                message: error.to_string(),
                duration_ms: started.elapsed().as_millis(),
            },
        );
    }
    let snapshot_after = staged_repository_snapshot(&root)?;
    if snapshot_after.identity.semantic_hash != snapshot_before.identity.semantic_hash
        || snapshot_after.identity.semantic_hash != paired.candidate_identity_after
    {
        return finish_failure(
            args,
            &snapshot_before,
            Some(identity),
            tool_summary,
            FailureOutcome {
                result: SpecPrecommitResultClass::StaleInput,
                exit_family: "stale_input",
                message: "the Git index changed during staged evaluation".to_string(),
                duration_ms: started.elapsed().as_millis(),
            },
        );
    }

    let evaluation = crate::spec_system_workspace::evaluate_paired_precommit_objectives(
        &paired,
        &Default::default(),
        false,
    );
    let result = result_class(&snapshot_before, &paired.candidate, &evaluation);
    let report = report_from_evaluation(
        &snapshot_before,
        &paired,
        &evaluation,
        result,
        identity,
        selection,
        started.elapsed().as_millis(),
    );
    emit_report(args, &root, &report)?;
    if matches!(
        report.result_class,
        SpecPrecommitResultClass::Passed
            | SpecPrecommitResultClass::FindingsAdvisory
            | SpecPrecommitResultClass::NotApplicable
    ) {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            report
                .error
                .as_deref()
                .unwrap_or("staged precommit posture is not green"),
        ))
    }
}

fn report_from_evaluation(
    snapshot: &StagedRepositorySnapshot,
    paired: &crate::spec_system_workspace::PairedSelfHostedGraphCompilation,
    evaluation: &PrecommitObjectiveEvaluation,
    result: SpecPrecommitResultClass,
    identity: CargoAllowToolIdentityV1,
    selection: ToolSelectionReceiptV1,
    duration_ms: u128,
) -> SpecPrecommitReportV1 {
    let findings = evaluation
        .findings
        .iter()
        .map(|finding| PrecommitFindingV1 {
            code: finding.code.as_str().to_string(),
            subject: finding.subject.clone(),
            posture: match finding.posture {
                PrecommitFindingPosture::Blocking => "blocking".to_string(),
                PrecommitFindingPosture::Advisory => "advisory".to_string(),
            },
            message: finding.message.clone(),
            action: finding.action.clone(),
        })
        .collect::<Vec<_>>();
    SpecPrecommitReportV1 {
        schema_id: SPEC_PRECOMMIT_SCHEMA_ID,
        schema_version: SPEC_PRECOMMIT_SCHEMA_VERSION,
        command: "check",
        phase: "precommit",
        profile: "spec-system",
        tool_identity: Some(identity),
        tool_selection: Some(tool_summary(&selection)),
        parent_commit: Some(paired.parent_identity.commit.clone()),
        parent_tree: Some(paired.parent_identity.tree.clone()),
        staged_identity_before: Some(paired.candidate_identity_before.clone()),
        staged_identity_after: Some(paired.candidate_identity_after.clone()),
        staged_changes: snapshot.changes.iter().map(staged_change).collect(),
        change_class: Some(evaluation.change_class.as_str().to_string()),
        findings,
        result_class: result,
        process_exit_family: exit_family_for(result),
        inventory_completeness: inventory_completeness(&paired.candidate),
        source_view_identity: paired.candidate.source_identity.clone(),
        tool_result_class: Some(format!("{:?}", selection.result)),
        duration_ms,
        remaining_gates: vec![
            "focused tests and pre-push proof remain outside this command",
            "full CI and independent review remain outside this command",
        ],
        error: None,
        claim_boundary: CLAIM_BOUNDARY,
    }
}

fn report_for_unmapped_staged_surface(
    snapshot: &StagedRepositorySnapshot,
    identity: CargoAllowToolIdentityV1,
    selection: Option<ToolSelectionSummaryV1>,
    duration_ms: u128,
) -> SpecPrecommitReportV1 {
    SpecPrecommitReportV1 {
        schema_id: SPEC_PRECOMMIT_SCHEMA_ID,
        schema_version: SPEC_PRECOMMIT_SCHEMA_VERSION,
        command: "check",
        phase: "precommit",
        profile: "spec-system",
        tool_identity: Some(identity),
        tool_selection: selection,
        parent_commit: snapshot.parent_commit.clone(),
        parent_tree: None,
        staged_identity_before: Some(snapshot.identity.semantic_hash.clone()),
        staged_identity_after: Some(snapshot.identity.semantic_hash.clone()),
        staged_changes: snapshot.changes.iter().map(staged_change).collect(),
        change_class: Some("unknown_or_mixed".to_string()),
        findings: vec![PrecommitFindingV1 {
            code: "precommit_unknown_staged_surface".to_string(),
            subject: "staged-candidate".to_string(),
            posture: "blocking".to_string(),
            message: "the staged candidate changed outside the self-hosted spec-system graph inputs".to_string(),
            action: "declare and map the affected implementation slice, or stage the governing spec-system inputs before evaluation".to_string(),
        }],
        result_class: SpecPrecommitResultClass::FindingsBlocking,
        process_exit_family: "blocking",
        inventory_completeness: snapshot_completeness(snapshot.completeness),
        source_view_identity: None,
        tool_result_class: None,
        duration_ms,
        remaining_gates: vec!["the affected staged source has no graph-backed objective mapping"],
        error: None,
        claim_boundary: CLAIM_BOUNDARY,
    }
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
    let cwd = env::current_dir()
        .map_err(|error| CargoAllowError::new(format!("failed to read cwd: {error}")))?;
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

fn current_executable() -> CargoAllowResult<PathBuf> {
    env::current_exe().map_err(|error| {
        CargoAllowError::new(format!("failed to resolve current executable: {error}"))
    })
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

fn staged_graph_inputs_changed(snapshot: &StagedRepositorySnapshot) -> bool {
    const GRAPH_INPUTS: [&str; 4] = [
        "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md",
        ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml",
        ".allow/spec-system/seams/runtime-promotion-validator-v1.toml",
        ".allow/spec-system/evidence/runtime-promotion-v1.toml",
    ];
    snapshot.changes.iter().any(|change| {
        change
            .path
            .as_deref()
            .map(|path| GRAPH_INPUTS.iter().any(|input| Path::new(input) == path))
            .unwrap_or(false)
            || change
                .previous_path
                .as_deref()
                .map(|path| GRAPH_INPUTS.iter().any(|input| Path::new(input) == path))
                .unwrap_or(false)
    })
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

fn inventory_completeness(
    candidate: &crate::spec_system_workspace::SelfHostedGraphCompilation,
) -> &'static str {
    if candidate.inventory.status == RustTestInventoryStatus::Partial {
        return "partial";
    }
    match candidate.file_inventory.completeness {
        InventoryCompleteness::Complete | InventoryCompleteness::Scoped => "complete",
        InventoryCompleteness::Partial => "partial",
        InventoryCompleteness::Fallback => "unsupported",
    }
}

fn result_class(
    snapshot: &StagedRepositorySnapshot,
    candidate: &crate::spec_system_workspace::SelfHostedGraphCompilation,
    evaluation: &PrecommitObjectiveEvaluation,
) -> SpecPrecommitResultClass {
    if snapshot.changes.is_empty() {
        return SpecPrecommitResultClass::NotApplicable;
    }
    if snapshot.completeness == StagedSnapshotCompleteness::Partial
        || candidate.inventory.status == RustTestInventoryStatus::Partial
        || candidate.file_inventory.completeness == InventoryCompleteness::Partial
    {
        return SpecPrecommitResultClass::PartialData;
    }
    if evaluation
        .findings
        .iter()
        .any(|finding| finding.posture == PrecommitFindingPosture::Blocking)
    {
        return SpecPrecommitResultClass::FindingsBlocking;
    }
    if evaluation
        .findings
        .iter()
        .any(|finding| finding.posture == PrecommitFindingPosture::Advisory)
    {
        return SpecPrecommitResultClass::FindingsAdvisory;
    }
    SpecPrecommitResultClass::Passed
}

fn error_result_class(error: &CargoAllowError) -> SpecPrecommitResultClass {
    if error.kind() == CargoAllowErrorKind::InvalidConfig
        || error.kind() == CargoAllowErrorKind::InvalidPolicy
    {
        SpecPrecommitResultClass::MalformedInput
    } else if error.kind() == CargoAllowErrorKind::Inventory
        && error.to_string().to_ascii_lowercase().contains("changed")
    {
        SpecPrecommitResultClass::StaleInput
    } else {
        SpecPrecommitResultClass::InstrumentFailure
    }
}

fn tool_failure_result(result: &ToolResultClass) -> SpecPrecommitResultClass {
    match result {
        ToolResultClass::ToolGenerationUnsupported
        | ToolResultClass::CandidateSchemaUnsupported => SpecPrecommitResultClass::Unsupported,
        ToolResultClass::ToolIdentityMismatch
        | ToolResultClass::ToolChangedDuringRun
        | ToolResultClass::ToolMissing => SpecPrecommitResultClass::StaleInput,
        ToolResultClass::PreviewToolNotAuthorized | ToolResultClass::MalformedToolIdentity => {
            SpecPrecommitResultClass::MalformedInput
        }
        ToolResultClass::ToolPrebuiltAndSelected => SpecPrecommitResultClass::Passed,
    }
}

fn exit_family_for(result: SpecPrecommitResultClass) -> &'static str {
    match result {
        SpecPrecommitResultClass::Passed
        | SpecPrecommitResultClass::FindingsAdvisory
        | SpecPrecommitResultClass::NotApplicable => "success",
        SpecPrecommitResultClass::FindingsBlocking => "blocking",
        SpecPrecommitResultClass::MalformedInput => "usage",
        _ => "instrument_failure",
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

fn tool_summary(selection: &ToolSelectionReceiptV1) -> ToolSelectionSummaryV1 {
    ToolSelectionSummaryV1 {
        result: format!("{:?}", selection.result),
        mode: format!("{:?}", selection.mode),
        executable_digest: Some(selection.executable_digest.clone()),
        channel: Some(format!("{:?}", selection.identity.channel)),
        preview_evidence: selection.preview_evidence,
    }
}

fn tool_summary_from_failure(
    mode: ToolSelectionMode,
    result: &ToolResultClass,
) -> ToolSelectionSummaryV1 {
    ToolSelectionSummaryV1 {
        result: format!("{result:?}"),
        mode: format!("{mode:?}"),
        executable_digest: None,
        channel: None,
        preview_evidence: matches!(mode, ToolSelectionMode::ExplicitToolUnderTest),
    }
}

pub(crate) struct DelegatedPrecommitOutcome {
    pub result_class: repo_protocol::ResultClassV1,
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
                repo_protocol::ResultClassV1::StaleInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::MalformedOutput => {
                repo_protocol::ResultClassV1::MalformedInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::WrongProduct
            | crate::intent_delegate::IntentDelegateFailureClass::WrongProtocol => {
                repo_protocol::ResultClassV1::MalformedInput
            }
            crate::intent_delegate::IntentDelegateFailureClass::Timeout => {
                repo_protocol::ResultClassV1::InstrumentFailure
            }
            crate::intent_delegate::IntentDelegateFailureClass::ProviderAbsent
            | crate::intent_delegate::IntentDelegateFailureClass::IdentityMismatch
            | crate::intent_delegate::IntentDelegateFailureClass::InstrumentFailure => {
                repo_protocol::ResultClassV1::InstrumentFailure
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
    let result_class = map_delegated_result_class(&outcome);
    let exit_family = if outcome.error.is_some() {
        "instrument_failure"
    } else {
        static_exit_family(&outcome.process_exit_family)
    };
    let remaining_gates = if outcome.unmapped_staged_surface {
        vec![
            "delegated via repo.analysis-receipt.v1",
            "provider reported unmapped staged surface; embedded graph evaluation skipped",
        ]
    } else {
        vec![
            "delegated via repo.analysis-receipt.v1",
            "provider obligation skeleton only; embedded evaluator not invoked",
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
            repo_protocol::ResultClassV1::StaleInput => SpecPrecommitResultClass::StaleInput,
            repo_protocol::ResultClassV1::MalformedInput => {
                SpecPrecommitResultClass::MalformedInput
            }
            _ => SpecPrecommitResultClass::InstrumentFailure,
        };
    }
    match outcome.result_class {
        repo_protocol::ResultClassV1::Completed => SpecPrecommitResultClass::Passed,
        repo_protocol::ResultClassV1::Findings => {
            if outcome.process_exit_family == "advisory" {
                SpecPrecommitResultClass::FindingsAdvisory
            } else {
                SpecPrecommitResultClass::FindingsBlocking
            }
        }
        repo_protocol::ResultClassV1::PartialData => SpecPrecommitResultClass::PartialData,
        repo_protocol::ResultClassV1::StaleInput => SpecPrecommitResultClass::StaleInput,
        repo_protocol::ResultClassV1::MalformedInput => SpecPrecommitResultClass::MalformedInput,
        repo_protocol::ResultClassV1::Unsupported => SpecPrecommitResultClass::Unsupported,
        repo_protocol::ResultClassV1::InstrumentFailure => {
            SpecPrecommitResultClass::InstrumentFailure
        }
        repo_protocol::ResultClassV1::NotProven
        | repo_protocol::ResultClassV1::Cancelled
        | repo_protocol::ResultClassV1::Conflict => SpecPrecommitResultClass::InstrumentFailure,
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
        let snapshot = staged_repository_snapshot(resolve_root(&RootArgs::default())?)?;
        let output = output_path("identity-handshake");
        let _ = fs::remove_file(&output);
        let mut args = check_args(Some(output.clone()), None);
        args.expect_staged_identity = Some(snapshot.identity.semantic_hash);
        let _ = cmd_spec_precommit(&args);
        let report: serde_json::Value = serde_json::from_str(&fs::read_to_string(&output)?)?;
        let _ = fs::remove_file(&output);
        if report
            .get("staged_identity_before")
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            return Err("precommit report did not retain the staged identity handshake".into());
        }
        Ok(())
    }
}
