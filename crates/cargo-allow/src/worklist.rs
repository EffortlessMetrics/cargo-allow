use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_match::{CheckMode, evaluate};

use crate::evidence_inventory::current_evidence_source_tree_files;
use crate::{
    EvidenceValidationMode, HumanJsonFormat, ProfileArg, SourceTreeReportContext, emit_text,
    load_read_only_world, report_config, spec_system,
};

#[path = "worklist_actions.rs"]
mod worklist_actions;
#[path = "worklist_advisories.rs"]
mod worklist_advisories;
#[path = "worklist_args.rs"]
mod worklist_args;
#[path = "worklist_evidence.rs"]
mod worklist_evidence;
#[path = "worklist_federation.rs"]
mod worklist_federation;
#[path = "worklist_item_kind.rs"]
mod worklist_item_kind;
#[path = "worklist_items.rs"]
mod worklist_items;
#[path = "worklist_priority.rs"]
mod worklist_priority;
#[path = "worklist_queue.rs"]
mod worklist_queue;
#[path = "worklist_render.rs"]
mod worklist_render;
#[path = "worklist_scoring.rs"]
mod worklist_scoring;
#[path = "worklist_types.rs"]
mod worklist_types;
pub(crate) use worklist_actions::{
    proof_commands, suggested_actions, suggested_actions_for_context,
    suggested_link_actions_for_context,
};
use worklist_advisories::work_items_from_policy_advisories;
pub(crate) use worklist_args::WorklistArgs;
use worklist_args::worklist_filters;
#[cfg(test)]
use worklist_evidence::work_items_from_evidence_diagnostics;
use worklist_evidence::work_items_from_evidence_diagnostics_with_source_tree_files;
use worklist_federation::work_items_from_federation_divergences;
#[cfg(test)]
pub(crate) use worklist_item_kind::WORK_ITEM_KINDS;
use worklist_items::work_items_from_outcomes;
#[cfg(test)]
pub(crate) use worklist_priority::{DIFFICULTY_LEVELS, RISK_LEVELS};
use worklist_queue::{filter_work_items, renumber_work_items, sort_work_items};
#[cfg(test)]
use worklist_render::render_worklist_human_with_context;
use worklist_render::{
    render_worklist_human_with_context_styled, render_worklist_json_with_context,
};
pub(crate) use worklist_scoring::work_item_kind;
#[cfg(test)]
use worklist_types::WorkItemLedger;
pub(super) use worklist_types::{WorkItem, WorkItemEvidenceReference};
use worklist_types::{WorklistContext, WorklistFilters};

#[cfg(test)]
use allow_core::{AllowConfig, FindingKind, MatchOutcome, MatchStatus};

pub(crate) fn cmd_worklist(args: &WorklistArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        reject_source_exception_options_for_profile(args)?;
        return spec_system::cmd_spec_system_worklist(spec_system::SpecSystemWorklistCommandArgs {
            root: &args.root,
            config: args.config.as_deref(),
            format_json: matches!(args.format, HumanJsonFormat::Json),
            output: args.output.as_deref(),
        });
    }

    let (root, cfg, findings, inventory_facts, federation) = load_read_only_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?
    .into_parts();
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let filters = worklist_filters(args);
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let mut items = work_items_from_outcomes(&report_cfg, &findings, &outcomes);
    items.extend(work_items_from_policy_advisories(
        &report_cfg,
        &findings,
        &outcomes,
        items.len() + 1,
    ));
    items.extend(work_items_from_evidence_diagnostics_with_source_tree_files(
        &root,
        &report_cfg,
        items.len() + 1,
        evidence_source_tree_files.as_ref(),
    ));
    items.extend(work_items_from_federation_divergences(
        &federation.divergences,
        items.len() + 1,
    ));
    let mut items = filter_work_items(items, filters);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = WorklistContext {
        inventory: source_context.inventory(),
        filters,
    };
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    // The JSON artifact is rendered for every format: it supplies the
    // relocation-stable semantic identity the summary reports, and it is the
    // command's own artifact when JSON was requested.
    let detail_json = render_worklist_json_with_context(&items, context);
    // Common operator grammar (#3149). The detailed worklist artifact remains
    // authoritative; this projection is additive and derived from the same
    // in-memory queue without rescanning source or re-evaluating matching.
    let summary = worklist_summary(
        &detail_json,
        &root,
        &source_context,
        &items,
        filters.any_active(),
        inventory_facts,
    )?;
    crate::core_command_router::write_summary_artifact(&root, &summary)?;

    let text = match args.format {
        HumanJsonFormat::Json => detail_json,
        HumanJsonFormat::Human => {
            let mut rendered =
                crate::core_command_summary::render_core_command_summary_human(&summary);
            rendered.push('\n');
            rendered.push_str(&render_worklist_human_with_context_styled(
                &items, context, style,
            ));
            rendered
        }
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

/// Build the common operator summary from the queue worklist already ranked.
///
/// The relocation-stable semantic identity comes from worklist's own JSON
/// artifact, exactly as `audit`, `check`, and `doctor` derive theirs, so the
/// summary never rescans source or re-evaluates matching to describe itself.
fn worklist_summary(
    detail_json: &str,
    root: &std::path::Path,
    source_context: &SourceTreeReportContext,
    items: &[WorkItem],
    filtered: bool,
    inventory_facts: crate::InventoryFacts,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    let semantic_identity =
        crate::core_command_router::canonical_semantic_identity(detail_json, Some(root))?;
    let completeness = crate::core_command_router::summary_completeness(&inventory_facts);
    let coverage_limitation = (completeness != effortless_repo_protocol::CompletenessV1::Complete)
        .then(|| crate::core_command_router::partial_coverage_reason(&inventory_facts));
    let inventory_source = source_context.inventory_source();

    let mut subject = crate::core_command_summary::CoreSourceSubjectV1::worktree(
        format!("local-repository:{semantic_identity}"),
        format!("worktree:{inventory_source}:current-unpinned"),
    );
    subject.limitations.push(
        "the current worktree result is not bound to a commit, tree, or Git-index identity"
            .to_string(),
    );

    crate::core_command_summary::core_command_summary_from_worklist(
        crate::core_command_summary::WorklistSummaryFactsV1 {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            subject,
            completeness,
            coverage_limitation,
            items: items.iter().map(summary_item).collect(),
            filtered,
            claim_boundary: effortless_repo_protocol::ClaimBoundaryV1::new(
                "cargo-allow queued source-exception maintenance work from current source-tree syntax and ledger posture only",
            )
            .with_limitations(vec![
                "cargo metadata, rustc, Clippy, build scripts, proc macros, tests, and repository code were not invoked"
                    .to_string(),
                "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                    .to_string(),
                "an empty queue does not prove the repository passes the no-new gate".to_string(),
            ]),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build core command summary: {error}"),
        )
    })
}

/// Project one already-ranked work item onto the fields the summary reads.
fn summary_item(item: &WorkItem) -> crate::core_command_summary::WorklistSummaryItemV1 {
    crate::core_command_summary::WorklistSummaryItemV1 {
        kind: item.kind.clone(),
        status: item.status,
        allow_id: item.allow_id.clone(),
        path: item.path.clone(),
        suggested_actions: item.suggested_actions.clone(),
    }
}

fn reject_source_exception_options_for_profile(args: &WorklistArgs) -> CargoAllowResult<()> {
    if args.kind.is_some() {
        return profile_option_error("--kind");
    }
    if args.family.is_some() {
        return profile_option_error("--family");
    }
    if args.item_kind.is_some() {
        return profile_option_error("--item-kind");
    }
    if args.status.is_some() {
        return profile_option_error("--status");
    }
    if args.allow_id.is_some() {
        return profile_option_error("--allow-id");
    }
    if args.path.is_some() {
        return profile_option_error("--path");
    }
    if args.source_package.is_some() {
        return profile_option_error("--source-package");
    }
    if args.owner.is_some() {
        return profile_option_error("--owner");
    }
    if args.classification.is_some() {
        return profile_option_error("--classification");
    }
    if args.baseline_debt {
        return profile_option_error("--baseline-debt");
    }
    if args.broad_scope {
        return profile_option_error("--broad-scope");
    }
    if args.risk.is_some() {
        return profile_option_error("--risk");
    }
    if args.difficulty.is_some() {
        return profile_option_error("--difficulty");
    }
    if args.missing_evidence {
        return profile_option_error("--missing-evidence");
    }
    if args.broken_evidence {
        return profile_option_error("--broken-evidence");
    }
    if args.weak_evidence {
        return profile_option_error("--weak-evidence");
    }
    if args.include_untracked {
        return profile_option_error("--include-untracked");
    }
    Ok(())
}

fn profile_option_error<T>(option: &str) -> CargoAllowResult<T> {
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Artifact,
        format!("{option} is not supported with --profile spec-system"),
    ))
}

#[cfg(test)]
pub(crate) fn sample_worklist_json_for_contract_test() -> String {
    let items = vec![WorkItem {
        id: "work-baseline-debt-0001".to_string(),
        kind: "baseline_debt".to_string(),
        exception_kind: Some("panic".to_string()),
        family: Some("unwrap".to_string()),
        owner: Some("core/parser".to_string()),
        classification: Some("baseline_debt".to_string()),
        reason: Some("Generated baseline debt requires human review.".to_string()),
        created: Some("2026-05-29".to_string()),
        review_after: Some("2026-06-29".to_string()),
        expires: Some("2027-08-29".to_string()),
        evidence_count: Some(1),
        selector_precision: Some(7),
        risk: worklist_priority::RISK_MEDIUM,
        difficulty: worklist_priority::DIFFICULTY_MEDIUM,
        status: allow_core::MatchStatus::BaselineDebt,
        allow_id: Some("allow-baseline".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        path: Some("src/lib.rs".to_string()),
        line: None,
        column: None,
        evidence_reference: None,
        source_package: Some("parser".to_string()),
        message: "allow-baseline is generated baseline debt and still needs human review"
            .to_string(),
        suggested_actions: vec![
            "replace generated baseline debt with a reviewed allow entry".to_string(),
            "or remove the underlying exception".to_string(),
        ],
        proof_commands: vec![
            "cargo-allow explain allow-baseline".to_string(),
            "cargo-allow list --allow-id allow-baseline --format json".to_string(),
            "cargo-allow worklist --allow-id allow-baseline --format json".to_string(),
        ],
        ledger: WorkItemLedger::default(),
    }];
    render_worklist_json_with_context(
        &items,
        WorklistContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_fallback",
                Some("fixtures/source-snapshot"),
                Some(5),
            ),
            filters: WorklistFilters::default(),
        },
    )
}

#[cfg(test)]
#[path = "worklist_action_tests.rs"]
mod action_tests;
#[cfg(test)]
#[path = "worklist_advisory_tests.rs"]
mod advisory_tests;
#[cfg(test)]
#[path = "worklist_cli_tests.rs"]
mod cli_tests;
#[cfg(test)]
#[path = "worklist_evidence_advisory_tests.rs"]
mod evidence_advisory_tests;
#[cfg(test)]
#[path = "worklist_filter_policy_tests.rs"]
mod filter_policy_tests;
#[cfg(test)]
#[path = "worklist_filter_source_tests.rs"]
mod filter_source_tests;
#[cfg(test)]
#[path = "worklist_filter_tests.rs"]
mod filter_tests;
#[cfg(test)]
#[path = "worklist_proof_command_tests.rs"]
mod proof_command_tests;
#[cfg(test)]
#[path = "worklist_render_context_tests.rs"]
mod render_context_tests;
#[cfg(test)]
#[path = "worklist_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "spec_system_worklist_tests.rs"]
mod spec_system_worklist_tests;
#[cfg(test)]
#[path = "worklist_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "worklist_tests.rs"]
mod tests;
