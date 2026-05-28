use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};

use crate::{
    SourceTreeReportContext, load_world_with_evidence_validation, report_config, write_file,
};

#[path = "worklist_actions.rs"]
mod worklist_actions;
#[path = "worklist_advisories.rs"]
mod worklist_advisories;
#[path = "worklist_args.rs"]
mod worklist_args;
#[path = "worklist_evidence.rs"]
mod worklist_evidence;
#[path = "worklist_items.rs"]
mod worklist_items;
#[path = "worklist_queue.rs"]
mod worklist_queue;
#[path = "worklist_render.rs"]
mod worklist_render;
#[path = "worklist_scoring.rs"]
mod worklist_scoring;
#[path = "worklist_types.rs"]
mod worklist_types;
pub(crate) use worklist_actions::{proof_commands, suggested_actions};
use worklist_advisories::work_items_from_policy_advisories;
pub(crate) use worklist_args::WorklistArgs;
use worklist_args::{WorklistFormat, worklist_filters};
use worklist_evidence::work_items_from_evidence_diagnostics;
use worklist_items::work_items_from_outcomes;
use worklist_queue::{filter_work_items, renumber_work_items, sort_work_items};
use worklist_render::{render_worklist_human_with_context, render_worklist_json_with_context};
pub(crate) use worklist_scoring::work_item_kind;
use worklist_types::{WorkItem, WorklistContext, WorklistFilters};

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
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
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
#[path = "worklist_advisory_tests.rs"]
mod advisory_tests;
#[cfg(test)]
#[path = "worklist_cli_tests.rs"]
mod cli_tests;
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
#[path = "worklist_render_context_tests.rs"]
mod render_context_tests;
#[cfg(test)]
#[path = "worklist_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "worklist_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "worklist_tests.rs"]
mod tests;
