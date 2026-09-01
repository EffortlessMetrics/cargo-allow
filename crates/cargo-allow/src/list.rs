use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, MatchStatus};
use allow_match::{CheckMode, evaluate};
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};
use serde_json::Value;

use crate::{
    EvidenceValidationMode, HumanJsonFormat, SourceTreeReportContext,
    core_command_summary::{
        CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
        CoreCommandSummaryV1, CoreSourceSubjectV1, build_core_command_summary,
        render_core_command_summary_human,
    },
    emit_text,
    evidence_inventory::current_evidence_source_tree_files,
    load_read_only_world,
};

#[path = "list_args.rs"]
mod list_args;
#[path = "list_filter.rs"]
mod list_filter;
#[path = "list_render.rs"]
mod list_render;
#[path = "list_rows.rs"]
mod list_rows;
#[path = "list_types.rs"]
mod list_types;
pub(crate) use list_args::ListArgs;
use list_args::list_filters;
#[cfg(test)]
use list_render::render_list_rows;
#[cfg(test)]
use list_render::render_list_rows_with_context;
#[cfg(test)]
use list_render::{render_list_rows_concise, render_list_rows_with_columns};
use list_render::{
    render_list_rows_concise_styled_with_width, render_list_rows_json,
    render_list_rows_with_columns_styled,
};
#[cfg(test)]
use list_rows::list_rows;
use list_rows::list_rows_with_source_tree_files;
use list_types::{ListContext, ListFilters, ListRow};

#[cfg(test)]
use crate::parse_kind_filter;
#[cfg(test)]
use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome};
#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts, _federation) = load_read_only_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?
    .into_parts();
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let all_rows = list_rows_with_source_tree_files(
        &root,
        &cfg,
        &findings,
        &outcomes,
        evidence_source_tree_files.as_ref(),
    );
    let filters = list_filters(args)?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ListContext {
        inventory: source_context.inventory(),
        kind_arg: args.kind.as_deref(),
    };
    let filtered_rows: Vec<_> = all_rows
        .iter()
        .filter(|row| list_filter::list_row_matches(row, &filters))
        .cloned()
        .collect();
    let summary = build_list_summary(
        &filtered_rows,
        &filters,
        context.inventory,
        args.root.root.as_deref(),
        args.config.as_deref(),
        args.include_untracked,
    )?;
    // Apply --offset/--limit pagination after filtering and before
    // rendering (#3173). Sort order is determined by the row builder; the
    // slice applies to the final sorted set.
    let rows = paginate_list_rows(filtered_rows, args.offset.unwrap_or(0), args.limit);
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let text = match args.format {
        HumanJsonFormat::Human => {
            let columns = list_columns(args)?;
            let detail = if args.wide || args.columns.is_some() {
                render_list_rows_with_columns_styled(&rows, &filters, context, &columns, style)
            } else {
                render_list_rows_concise_styled_with_width(
                    &rows,
                    &filters,
                    context,
                    &columns,
                    style,
                    list_args::concise_width(args),
                )
            };
            format!("{}\n{detail}", render_core_command_summary_human(&summary))
        }
        HumanJsonFormat::Json => {
            let detail = render_list_rows_json(&rows, &filters, context);
            add_core_summary_to_list_json(&detail, &summary)?
        }
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

fn paginate_list_rows(rows: Vec<ListRow>, offset: usize, limit: Option<usize>) -> Vec<ListRow> {
    rows.into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect()
}

fn build_list_summary(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    inventory: allow_report::InventoryContext<'_>,
    root: Option<&std::path::Path>,
    config: Option<&std::path::Path>,
    include_untracked: bool,
) -> CargoAllowResult<CoreCommandSummaryV1> {
    let matched_rows = rows
        .iter()
        .filter(|row| list_filter::list_row_matches(row, filters))
        .count();
    let unhealthy_rows = rows
        .iter()
        .filter(|row| list_filter::list_row_matches(row, filters))
        .filter(|row| row.status != MatchStatus::Matched)
        .count();
    let complete = matches!(inventory.completeness, Some("complete" | "scoped"));
    let (result_class, posture, completeness, currentness, reason) = if !complete {
        (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CompletenessV1::Partial,
            CurrentnessV1::PartialOrUnavailable,
            CoreCommandReasonV1 {
                code: "list.partial_coverage".to_string(),
                message: "the source inventory is incomplete; this list cannot establish a complete ledger view".to_string(),
            },
        )
    } else if matched_rows == 0 && has_active_list_filter(filters) {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::NotApplicable,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "list.no_filter_matches".to_string(),
                message: "no allow entries matched the requested filters".to_string(),
            },
        )
    } else if matched_rows == 0 {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::NotApplicable,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "list.no_entries".to_string(),
                message: "no allow entries are configured".to_string(),
            },
        )
    } else if unhealthy_rows > 0 {
        (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Advisory,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "list.ledger_attention".to_string(),
                message: format!(
                    "{unhealthy_rows} listed entr{} require{} lifecycle or evidence attention",
                    if unhealthy_rows == 1 { "y" } else { "ies" },
                    if unhealthy_rows == 1 { "s" } else { "" },
                ),
            },
        )
    } else {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::NotApplicable,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
            CoreCommandReasonV1 {
                code: "list.entries_available".to_string(),
                message: format!(
                    "{matched_rows} allow entr{} available for inspection",
                    if matched_rows == 1 { "y" } else { "ies" }
                ),
            },
        )
    };

    let primary_action = if !complete {
        Some(
            CoreCommandActionV1::command(
                "list.diagnose_coverage",
                "Diagnose coverage",
                "cargo-allow",
                context_command_args("doctor", root, config, false),
            )
            .with_contract(
                "the list inventory is incomplete",
                "the inventory limitation is explained without modifying the repository",
                "doctor remains read-only and does not authorize policy entries",
            ),
        )
    } else if matched_rows == 1 {
        let id = rows
            .iter()
            .find(|row| list_filter::list_row_matches(row, filters))
            .map(|row| row.id.clone())
            .unwrap_or_default();
        Some(
            CoreCommandActionV1::command(
                "list.explain_entry",
                "Explain the matching entry",
                "cargo-allow",
                context_command_args_with_prefix(
                    vec!["explain".to_string(), id],
                    root,
                    config,
                    include_untracked,
                ),
            )
            .with_contract(
                "exactly one entry matched the current list query",
                "the selected entry's lifecycle, evidence, and next steps are shown",
                "explain is read-only and does not change source or policy",
            ),
        )
    } else if matched_rows == 0 && has_active_list_filter(filters) {
        Some(
            CoreCommandActionV1::command(
                "list.remove_filters",
                "Inspect the unfiltered ledger",
                "cargo-allow",
                context_command_args("list", root, config, include_untracked),
            )
            .with_contract(
                "the current filter matched no entries",
                "the complete available ledger projection is shown",
                "list remains read-only and does not infer a replacement filter",
            ),
        )
    } else if matched_rows == 0 {
        Some(
            CoreCommandActionV1::command(
                "list.begin_adoption",
                "Review adoption guidance",
                "cargo-allow",
                context_command_args("adopt", root, config, include_untracked),
            )
            .with_contract(
                "the ledger is empty",
                "the supported adoption route is explained before any write is selected",
                "adopt is a read-only plan and does not approve exceptions",
            ),
        )
    } else {
        None
    };

    let mut limitations = vec![
        "list reports source-tree syntax and ledger posture only".to_string(),
        "the list command does not prove compiled or runtime behavior".to_string(),
    ];
    if inventory.source_identity.is_none() {
        limitations.push("the worktree is not bound to an immutable source identity".to_string());
    }
    let portable_identity = inventory
        .source_identity
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "worktree:{}:{}:{}",
                inventory.source, inventory.scope, inventory.scanner
            )
        });
    let subject = CoreSourceSubjectV1 {
        kind: inventory
            .source_identity
            .map(|_| crate::core_command_summary::CoreSourceSubjectKindV1::Index)
            .unwrap_or(crate::core_command_summary::CoreSourceSubjectKindV1::Worktree),
        repository_identity: format!("local-repository:{}", inventory.source),
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

    build_core_command_summary(crate::core_command_summary::CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: "list".to_string(),
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
        next_proof: None,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "cargo-allow listed the selected source-tree ledger view; it did not authorize or mutate entries",
        )
        .with_limitations(limitations),
    })
    .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::Internal, error))
}

fn context_command_args(
    command: &str,
    root: Option<&std::path::Path>,
    config: Option<&std::path::Path>,
    include_untracked: bool,
) -> Vec<String> {
    context_command_args_with_prefix(vec![command.to_string()], root, config, include_untracked)
}

fn context_command_args_with_prefix(
    mut args: Vec<String>,
    root: Option<&std::path::Path>,
    config: Option<&std::path::Path>,
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

fn has_active_list_filter(filters: &ListFilters<'_>) -> bool {
    filters.kind.is_some()
        || filters.family.is_some()
        || filters.owner.is_some()
        || filters.classification.is_some()
        || filters.path.is_some()
        || filters.source_package.is_some()
        || filters.allow_id.is_some()
        || filters.status.is_some()
        || filters.expired
        || filters.review_due
        || filters.stale
        || filters.location_drift
        || filters.baseline_debt
        || filters.broad_scope
        || filters.missing_evidence
        || filters.broken_evidence
        || filters.weak_evidence
}

fn add_core_summary_to_list_json(
    detail: &str,
    summary: &CoreCommandSummaryV1,
) -> CargoAllowResult<String> {
    let mut document: Value = serde_json::from_str(detail).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to parse list JSON for core summary projection: {error}"),
        )
    })?;
    let summary = serde_json::to_value(summary).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to serialize list core summary: {error}"),
        )
    })?;
    document
        .as_object_mut()
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                "list JSON root must be an object",
            )
        })?
        .insert("core_command_summary".to_string(), summary);
    serde_json::to_string_pretty(&document)
        .map(|json| format!("{json}\n"))
        .map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("failed to render list JSON with core summary: {error}"),
            )
        })
}

fn list_columns(args: &ListArgs) -> CargoAllowResult<Vec<allow_report::ListColumn>> {
    if args.wide {
        return Ok(allow_report::ListColumn::ALL.to_vec());
    }
    args.columns
        .as_deref()
        .map(allow_report::ListColumn::parse_csv)
        .transpose()
        .map_err(|error| {
            allow_core::CargoAllowError::with_kind(allow_core::CargoAllowErrorKind::Usage, error)
        })
        .map(|columns| columns.unwrap_or_else(|| allow_report::ListColumn::DEFAULT.to_vec()))
}

#[cfg(test)]
pub(crate) fn sample_list_json_for_contract_test() -> String {
    let row = ListRow {
        id: "allow-json".to_string(),
        status: MatchStatus::BaselineDebt,
        matches: 1,
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "baseline_debt".to_string(),
        scope: "src/lib.rs".to_string(),
        source_package: Some("allow-core".to_string()),
        evidence_count: 2,
        broken_evidence_references: 1,
        weak_evidence_references: 1,
        selector_precision: 7,
        broad_scope: false,
        review_after: "2026-09-01".to_string(),
        expires: "2026-12-01".to_string(),
        reason: "reason".to_string(),
    };
    let filters = ListFilters {
        kind: Some(
            parse_kind_filter("panic")
                .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter: {err}"))),
        ),
        family: Some("unwrap"),
        owner: Some("parser"),
        classification: Some("baseline_debt"),
        path: Some("src/lib.rs"),
        source_package: Some("allow-core"),
        allow_id: Some("allow-json"),
        status: Some("baseline_debt"),
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: true,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: true,
        weak_evidence: true,
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(46),
        ),
        kind_arg: Some("panic"),
    };
    render_list_rows_json(&[row], &filters, context)
}

#[cfg(test)]
#[path = "list_filter_policy_tests.rs"]
mod filter_policy_tests;
#[cfg(test)]
#[path = "list_filter_source_tests.rs"]
mod filter_source_tests;
#[cfg(test)]
#[path = "list_filter_tests.rs"]
mod filter_tests;
#[cfg(test)]
#[path = "list_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
