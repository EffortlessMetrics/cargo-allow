use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};

use crate::{SourceTreeReportContext, emit_text, load_world};

mod args;
mod filter;
mod render;
mod rows;
mod types;
pub(crate) use args::ListArgs;
use args::{ListFormat, list_filters};
use render::{render_list_rows, render_list_rows_json};
use rows::list_rows;
use types::{ListContext, ListFilters, ListRow};

#[cfg(test)]
use crate::parse_kind_filter;
#[cfg(test)]
use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus};
#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let rows = list_rows(&cfg, &findings, &outcomes);
    let filters = list_filters(args)?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ListContext {
        inventory: source_context.inventory(),
        kind_arg: args.kind.as_deref(),
    };
    let text = match args.format {
        ListFormat::Human => render_list_rows(&rows, &filters),
        ListFormat::Json => render_list_rows_json(&rows, &filters, context),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
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
        status: Some("baseline_debt"),
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: true,
        broad_scope: false,
        missing_evidence: false,
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
mod filter_policy_tests;
#[cfg(test)]
mod filter_source_tests;
#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
