use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};

use crate::{
    SourceTreeReportContext, emit_text, load_world_with_evidence_validation, report_config,
};

mod actions;
mod advisories;
mod args;
mod evidence;
mod items;
mod queue;
mod render;
mod scoring;
mod types;
pub(crate) use actions::{proof_commands, suggested_actions};
use advisories::work_items_from_policy_advisories;
pub(crate) use args::WorklistArgs;
use args::{WorklistFormat, worklist_filters};
use evidence::work_items_from_evidence_diagnostics;
use items::work_items_from_outcomes;
use queue::{filter_work_items, renumber_work_items, sort_work_items};
use render::{render_worklist_human_with_context, render_worklist_json_with_context};
pub(crate) use scoring::work_item_kind;
use types::{WorkItem, WorklistContext, WorklistFilters};

#[cfg(test)]
use allow_core::{AllowConfig, FindingKind, MatchOutcome, MatchStatus};

pub(crate) fn cmd_worklist(args: &WorklistArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_validation(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        false,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let mut items = work_items_from_outcomes(&report_cfg, &findings, &outcomes);
    items.extend(work_items_from_policy_advisories(
        &report_cfg,
        &findings,
        &outcomes,
        items.len() + 1,
    ));
    items.extend(work_items_from_evidence_diagnostics(
        &root,
        &report_cfg,
        items.len() + 1,
    ));
    let filters = worklist_filters(args);
    let mut items = filter_work_items(items, filters);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = WorklistContext {
        inventory: source_context.inventory(),
        filters,
    };
    let text = match args.format {
        WorklistFormat::Json => render_worklist_json_with_context(&items, context),
        WorklistFormat::Human => render_worklist_human_with_context(&items, context),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_worklist_json_for_contract_test() -> String {
    let items = Vec::new();
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
mod advisory_tests;
#[cfg(test)]
mod cli_tests;
#[cfg(test)]
mod filter_policy_tests;
#[cfg(test)]
mod filter_source_tests;
#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod render_context_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
