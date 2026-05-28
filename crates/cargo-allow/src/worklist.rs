use allow_core::{CargoAllowResult, MatchStatus};
use allow_match::{CheckMode, evaluate};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{
    RootArgs, load_world_with_evidence_validation, report_config, source_tree_root_text, write_file,
};

#[path = "worklist_actions.rs"]
mod worklist_actions;
#[path = "worklist_items.rs"]
mod worklist_items;
#[path = "worklist_queue.rs"]
mod worklist_queue;
#[path = "worklist_render.rs"]
mod worklist_render;
pub(crate) use worklist_actions::{proof_commands, suggested_actions};
pub(crate) use worklist_items::work_item_kind;
use worklist_items::{
    work_items_from_evidence_diagnostics, work_items_from_outcomes,
    work_items_from_policy_advisories,
};
use worklist_queue::{filter_work_items, renumber_work_items, sort_work_items};
use worklist_render::{render_worklist_human_with_context, render_worklist_json_with_context};

#[cfg(test)]
use allow_core::{AllowConfig, FindingKind, MatchOutcome};

#[derive(Debug, Clone, Parser)]
pub(crate) struct WorklistArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Filter work items by scanner or policy family.
    #[arg(long)]
    family: Option<String>,
    /// Filter work items by queue item kind, such as stale_allow or baseline_debt.
    #[arg(long)]
    item_kind: Option<String>,
    /// Filter work items by match status.
    #[arg(
        long,
        value_parser = [
            "matched",
            "new",
            "stale",
            "expired",
            "review_due",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt"
        ]
    )]
    status: Option<String>,
    /// Filter work items by durable allow entry ID.
    #[arg(long)]
    allow_id: Option<String>,
    /// Filter work items by source-tree path or path prefix.
    #[arg(long)]
    path: Option<String>,
    /// Filter work items by scanner-provided source-tree package context.
    #[arg(long)]
    source_package: Option<String>,
    /// Filter work items by policy owner.
    #[arg(long)]
    owner: Option<String>,
    /// Filter work items by policy classification.
    #[arg(long)]
    classification: Option<String>,
    /// Include only generated baseline debt work items.
    #[arg(long)]
    baseline_debt: bool,
    /// Include only broad source-tree scope advisory work items.
    #[arg(long)]
    broad_scope: bool,
    /// Filter work items by risk.
    #[arg(long, value_parser = ["low", "medium", "high"])]
    risk: Option<String>,
    /// Filter work items by estimated difficulty.
    #[arg(long, value_parser = ["small", "medium"])]
    difficulty: Option<String>,
    /// Include only policy-backed work items with no evidence references.
    #[arg(long)]
    missing_evidence: bool,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = WorklistFormat::Json)]
    format: WorklistFormat,
    /// Write worklist to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WorklistFormat {
    Human,
    Json,
}

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
    let filters = WorklistFilters {
        kind: args.kind.as_deref(),
        family: args.family.as_deref(),
        item_kind: args.item_kind.as_deref(),
        status: args.status.as_deref(),
        allow_id: args.allow_id.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        risk: args.risk.as_deref(),
        difficulty: args.difficulty.as_deref(),
        missing_evidence: args.missing_evidence,
    };
    let mut items = filter_work_items(items, filters);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);
    let root_text = source_tree_root_text(&root);
    let context = WorklistContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkItem {
    id: String,
    kind: String,
    exception_kind: Option<String>,
    family: Option<String>,
    owner: Option<String>,
    classification: Option<String>,
    reason: Option<String>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
    evidence_count: Option<usize>,
    risk: &'static str,
    difficulty: &'static str,
    status: MatchStatus,
    allow_id: Option<String>,
    finding_index: Option<usize>,
    path: Option<String>,
    source_package: Option<String>,
    message: String,
    suggested_actions: Vec<String>,
    proof_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct WorklistContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
    filters: WorklistFilters<'a>,
}

#[derive(Debug, Clone, Copy, Default)]
struct WorklistFilters<'a> {
    kind: Option<&'a str>,
    family: Option<&'a str>,
    item_kind: Option<&'a str>,
    status: Option<&'a str>,
    allow_id: Option<&'a str>,
    path: Option<&'a str>,
    source_package: Option<&'a str>,
    owner: Option<&'a str>,
    classification: Option<&'a str>,
    baseline_debt: bool,
    broad_scope: bool,
    risk: Option<&'a str>,
    difficulty: Option<&'a str>,
    missing_evidence: bool,
}

impl Default for WorklistContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            filters: WorklistFilters::default(),
        }
    }
}

#[cfg(test)]
pub(crate) fn sample_worklist_json_for_contract_test() -> String {
    let items = Vec::new();
    render_worklist_json_with_context(
        &items,
        WorklistContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(5),
            filters: WorklistFilters::default(),
        },
    )
}

#[cfg(test)]
#[path = "worklist_advisory_tests.rs"]
mod advisory_tests;
#[cfg(test)]
#[path = "worklist_filter_tests.rs"]
mod filter_tests;
#[cfg(test)]
#[path = "worklist_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "worklist_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "worklist_tests.rs"]
mod tests;
