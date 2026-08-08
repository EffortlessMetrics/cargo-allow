use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    MatchOutcome,
};
use allow_match::{CheckMode, evaluate, score_match};
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};
use serde_json::Value;
use std::path::Path;

use crate::{
    EvidenceValidationMode, HumanJsonFormat, ProfileArg, SourceTreeReportContext,
    core_command_summary::{
        CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
        CoreCommandSummaryInputV1, CoreCommandSummaryV1, CoreSourceSubjectKindV1,
        CoreSourceSubjectV1, build_core_command_summary, render_core_command_summary_human,
    },
    emit_text,
    evidence_inventory::current_evidence_source_tree_files,
    load_world_with_evidence_mode, spec_system,
};

#[path = "explain_args.rs"]
mod explain_args;
#[path = "explain_render.rs"]
mod explain_render;
#[path = "explain_steps.rs"]
mod explain_steps;
#[path = "explain_types.rs"]
mod explain_types;
pub(crate) use explain_args::ExplainArgs;
use explain_render::{
    explain_reference_attention_for_source_tree, render_explain_entry_json_with_steps,
    render_explain_entry_styled_with_steps,
};
#[cfg(test)]
use explain_render::{render_explain_entry_json, render_explain_entry_styled};
use explain_steps::explain_next_steps;
pub(super) use explain_types::ExplainContext;

pub(crate) fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if args.include_untracked {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--include-untracked is not supported with --profile spec-system; remove --include-untracked or drop --profile spec-system",
            ));
        }
        return spec_system::cmd_spec_system_explain(spec_system::SpecSystemExplainCommandArgs {
            artifact_id: &args.id,
            root: &args.root,
            config: args.config.as_deref(),
            format_json: matches!(args.format, HumanJsonFormat::Json),
            output: args.output.as_deref(),
        });
    }

    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    // Normalize numeric shorthand: "42" → "allow-0042", matching the ledger's
    // zero-padded id format (#3166).
    let normalized_id = normalize_allow_id_shorthand(&args.id);
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == normalized_id)
        .ok_or_else(|| missing_allow_entry_error(&normalized_id))?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ExplainContext {
        inventory: source_context.inventory(),
    };
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let (matching_findings, outcomes) = explain_entry_state(&cfg, entry, &findings);
    let references = explain_reference_attention_for_source_tree(
        &root,
        entry,
        evidence_source_tree_files.as_ref(),
    );
    let (suggested_actions, proof_commands) =
        explain_next_steps(entry, &matching_findings, &outcomes, references);
    let detail_json = render_explain_entry_json_with_steps(
        &root,
        entry,
        &matching_findings,
        &outcomes,
        explain_render::ExplainRenderOptions::with_steps(
            evidence_source_tree_files.as_ref(),
            context,
            &suggested_actions,
            &proof_commands,
        ),
    );
    let summary = build_explain_summary(
        entry,
        &outcomes,
        context.inventory,
        args.root.root.as_deref(),
        args.config.as_deref(),
        args.include_untracked,
        ExplainSummaryProjection {
            suggested_actions: &suggested_actions,
            proof_commands: &proof_commands,
        },
    )?;
    let text = match args.format {
        HumanJsonFormat::Human => {
            let detail = render_explain_entry_styled_with_steps(
                &root,
                entry,
                &matching_findings,
                &outcomes,
                style,
                explain_render::ExplainRenderOptions::with_steps(
                    evidence_source_tree_files.as_ref(),
                    context,
                    &suggested_actions,
                    &proof_commands,
                ),
            );
            format!("{}\n{detail}", render_core_command_summary_human(&summary))
        }
        HumanJsonFormat::Json => add_core_summary_to_explain_json(&detail_json, &summary)?,
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

struct ExplainSummaryProjection<'a> {
    suggested_actions: &'a [String],
    proof_commands: &'a [String],
}

fn build_explain_summary(
    entry: &AllowEntry,
    outcomes: &[MatchOutcome],
    inventory: allow_report::InventoryContext<'_>,
    root_arg: Option<&Path>,
    config: Option<&Path>,
    include_untracked: bool,
    projection: ExplainSummaryProjection<'_>,
) -> CargoAllowResult<CoreCommandSummaryV1> {
    let complete = matches!(inventory.completeness, Some("complete" | "scoped"));
    let attention = outcomes
        .iter()
        .filter(|outcome| outcome.status != allow_core::MatchStatus::Matched)
        .collect::<Vec<_>>();
    let blocking_attention = attention
        .iter()
        .any(|outcome| outcome.status.is_failure_in_no_new());
    let requires_attention = !attention.is_empty()
        || !projection.suggested_actions.is_empty()
        || !projection.proof_commands.is_empty();
    let (result_class, posture, completeness, currentness, reason) = if !complete {
        (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CompletenessV1::Partial,
            CurrentnessV1::PartialOrUnavailable,
            CoreCommandReasonV1 {
                code: "explain.partial_coverage".to_string(),
                message: "the source inventory is incomplete; this explanation cannot establish a complete entry posture".to_string(),
            },
        )
    } else if !requires_attention {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "explain.entry_matched".to_string(),
                message: format!(
                    "allow entry `{}` matched the selected source findings",
                    entry.id
                ),
            },
        )
    } else {
        (
            ResultClassV1::Findings,
            if blocking_attention {
                CoreCommandPostureV1::Blocking
            } else {
                CoreCommandPostureV1::Advisory
            },
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "explain.entry_requires_attention".to_string(),
                message: format!(
                    "allow entry `{}` has {} outcome(s) or deterministic next step(s) requiring attention",
                    entry.id,
                    attention.len(),
                ),
            },
        )
    };

    let primary_action = if !complete {
        Some(
            CoreCommandActionV1::command(
                "explain.diagnose_coverage",
                "Diagnose coverage",
                "cargo-allow",
                explain_context_args("doctor", root_arg, config, false),
            )
            .with_contract(
                "the explanation inventory is incomplete",
                "the inventory limitation is explained without modifying the repository",
                "doctor remains read-only, uses its default inventory scope, and does not authorize policy entries",
            ),
        )
    } else {
        projection.suggested_actions.first().map(|action| {
            CoreCommandActionV1::decision(
                "explain.inspect_worklist",
                "Inspect entry repair guidance",
            )
            .with_contract(
                action,
                "the exact typed maintenance and repair guidance for this entry is shown",
                "the suggested action is guidance and does not mutate source or policy",
            )
        })
    };

    let next_proof = projection.proof_commands.first().map(|command| {
        let mut parts = command.split_ascii_whitespace();
        let program = parts.next().unwrap_or("cargo-allow");
        let args = parts.map(str::to_string).collect::<Vec<_>>();
        CoreCommandActionV1::command(
            "explain.next_proof",
            "Run the next proof command",
            program,
            args,
        )
        .with_contract(
            "the detailed explanation emitted a deterministic proof command",
            "the selected proof command is available for operator execution",
            "cargo-allow does not execute the proof command as part of explain",
        )
    });
    let repository_identity = inventory
        .source_identity
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "worktree:{}:{}:{}",
                inventory.source, inventory.scope, inventory.scanner
            )
        });
    let portable_identity = inventory
        .source_identity
        .map(str::to_string)
        .unwrap_or_else(|| format!("worktree:{}:current-unpinned", inventory.source));
    let subject = CoreSourceSubjectV1 {
        kind: inventory
            .source_identity
            .map(|_| CoreSourceSubjectKindV1::Index)
            .unwrap_or(CoreSourceSubjectKindV1::Worktree),
        repository_identity: format!("local-repository:{repository_identity}"),
        portable_identity,
        base: None,
        head: None,
        paths: Vec::new(),
        limitations: if inventory.source_identity.is_none() {
            vec![
                "the current worktree result is not bound to a commit or index identity"
                    .to_string(),
            ]
        } else {
            Vec::new()
        },
    };
    let mut limitations = vec![
        "explain evaluates one selected source-tree ledger entry and its matching findings only"
            .to_string(),
        "the explain command does not prove compiled or runtime behavior".to_string(),
    ];
    if inventory.source_identity.is_none() {
        limitations.push("the worktree is not bound to an immutable source identity".to_string());
    }
    if !complete && include_untracked {
        limitations.push(
            "the partial-coverage doctor action uses its default inventory because doctor does not support --include-untracked".to_string(),
        );
    }

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: "explain".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness,
        reason,
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: CoreCommandEffectsV1::read_only(vec![
            "does not modify source, policy, Git, hooks, workflows, or GitHub settings".to_string(),
            "does not execute repository code or external evidence tools".to_string(),
        ]),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "cargo-allow explained one selected source-tree ledger entry and its observed matching outcomes; it did not authorize or mutate the entry",
        )
        .with_limitations(limitations),
    })
    .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::Internal, error))
}

fn explain_context_args(
    command: &str,
    root: Option<&Path>,
    config: Option<&Path>,
    include_untracked: bool,
) -> Vec<String> {
    explain_context_args_with_prefix(vec![command.to_string()], root, config, include_untracked)
}

fn explain_context_args_with_prefix(
    mut args: Vec<String>,
    root: Option<&Path>,
    config: Option<&Path>,
    include_untracked: bool,
) -> Vec<String> {
    if let Some(root) = root {
        args.extend(["--root".to_string(), root.to_string_lossy().into_owned()]);
    }
    if let Some(config) = config {
        args.extend([
            "--config".to_string(),
            config.to_string_lossy().into_owned(),
        ]);
    }
    if include_untracked {
        args.push("--include-untracked".to_string());
    }
    args
}

fn add_core_summary_to_explain_json(
    detail: &str,
    summary: &CoreCommandSummaryV1,
) -> CargoAllowResult<String> {
    let mut document: Value = serde_json::from_str(detail).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to parse explain JSON for core summary projection: {error}"),
        )
    })?;
    let summary = serde_json::to_value(summary).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to serialize explain core summary: {error}"),
        )
    })?;
    document
        .as_object_mut()
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                "explain JSON root must be an object",
            )
        })?
        .insert("core_command_summary".to_string(), summary);
    serde_json::to_string_pretty(&document)
        .map(|json| format!("{json}\n"))
        .map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("failed to render explain JSON with core summary: {error}"),
            )
        })
}

fn missing_allow_entry_error(id: &str) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!("no allow entry `{id}`; run `cargo-allow list` to see valid IDs"),
    )
}

/// Normalize an allow-entry id shorthand for lookup.
///
/// If `raw` is purely numeric, expands to `allow-{raw:04}` (zero-padded to 4
/// digits, matching the ledger's standard id format). Non-numeric ids pass
/// through unchanged. So "42" → "allow-0042", "allow-0272" → "allow-0272".
fn normalize_allow_id_shorthand(raw: &str) -> String {
    if raw.chars().all(|c| c.is_ascii_digit()) && !raw.is_empty() {
        let n: usize = raw.parse().unwrap_or(0);
        format!("allow-{n:04}")
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
fn explain_entry_text(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> String {
    explain_entry_text_with_source_tree_files(
        root,
        cfg,
        entry,
        findings,
        None,
        allow_report::Style::PLAIN,
    )
}

#[cfg(test)]
fn explain_entry_text_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    evidence_source_tree_files: Option<&std::collections::BTreeSet<String>>,
    style: allow_report::Style,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_styled(
        root,
        entry,
        &matching_findings,
        &outcomes,
        evidence_source_tree_files,
        style,
    )
}

#[cfg(test)]
fn explain_entry_json(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    context: ExplainContext<'_>,
) -> String {
    explain_entry_json_with_source_tree_files(root, cfg, entry, findings, None, context)
}

#[cfg(test)]
fn explain_entry_json_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    evidence_source_tree_files: Option<&std::collections::BTreeSet<String>>,
    context: ExplainContext<'_>,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_json(
        root,
        entry,
        &matching_findings,
        &outcomes,
        evidence_source_tree_files,
        context,
    )
}

fn explain_entry_state(
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> (Vec<Finding>, Vec<MatchOutcome>) {
    let matching_findings = findings
        .iter()
        .filter(|finding| score_match(entry, finding).is_some())
        .cloned()
        .collect::<Vec<_>>();
    let mut single_entry_cfg = cfg.clone();
    single_entry_cfg.allow = vec![entry.clone()];
    let outcomes = evaluate(&single_entry_cfg, &matching_findings, CheckMode::NoNew);
    (matching_findings, outcomes)
}

#[cfg(test)]
pub(crate) fn sample_explain_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    let mut cfg = AllowConfig::empty();
    let entry = AllowEntry {
        id: "allow-json".to_string(),
        kind: FindingKind::NonRustFile,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };
    cfg.allow.push(entry.clone());
    let finding = Finding {
        kind: FindingKind::NonRustFile,
        family: None,
        path: PathBuf::from("tracked.file"),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "test finding".to_string(),
        ledger: None,
    };
    explain_entry_json_with_source_tree_files(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        None,
        ExplainContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(47),
            ),
        },
    )
}

#[cfg(test)]
#[path = "explain_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "explain_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "explain_tests.rs"]
mod tests;
