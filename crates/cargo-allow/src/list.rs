use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};

use crate::{
    EvidenceValidationMode, HumanJsonFormat, SourceTreeReportContext, emit_text,
    evidence_inventory::current_evidence_source_tree_files, load_world_with_evidence_mode,
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
    render_list_rows_concise_styled, render_list_rows_json, render_list_rows_with_columns_styled,
};
#[cfg(test)]
use list_rows::list_rows;
use list_rows::list_rows_with_source_tree_files;
use list_types::{ListContext, ListFilters, ListRow};

#[cfg(test)]
use crate::parse_kind_filter;
#[cfg(test)]
use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus};
#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let rows = list_rows_with_source_tree_files(
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
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let text = match args.format {
        HumanJsonFormat::Human => {
            let columns = list_columns(args)?;
            if args.wide || args.columns.is_some() {
                render_list_rows_with_columns_styled(&rows, &filters, context, &columns, style)
            } else {
                render_list_rows_concise_styled(&rows, &filters, context, &columns, style)
            }
        }
        HumanJsonFormat::Json => render_list_rows_json(&rows, &filters, context),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

fn list_columns(args: &ListArgs) -> CargoAllowResult<Vec<allow_report::ListColumn>> {
    if args.wide {
        return Ok(allow_report::ListColumn::ALL.to_vec());
    }
    args.columns
        .as_deref()
        .map(allow_report::ListColumn::parse_csv)
        .transpose()
        .map_err(allow_core::CargoAllowError::new)
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
