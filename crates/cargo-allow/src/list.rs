use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, load_world, parse_kind_filter, source_tree_root_text, write_file};

#[path = "list_render.rs"]
mod list_render;
#[path = "list_rows.rs"]
mod list_rows;
#[path = "list_types.rs"]
mod list_types;
use list_render::{render_list_rows, render_list_rows_json};
use list_rows::list_rows;
use list_types::{ListContext, ListFilters, ListRow};

#[cfg(test)]
use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter allow entries by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Filter allow entries by scanner or policy family.
    #[arg(long)]
    family: Option<String>,
    /// Filter allow entries by owner.
    #[arg(long)]
    owner: Option<String>,
    /// Filter allow entries by classification.
    #[arg(long)]
    classification: Option<String>,
    /// Filter allow entries by source-tree path or path prefix.
    #[arg(long)]
    path: Option<String>,
    /// Filter allow entries by scanner-provided source-tree package context.
    #[arg(long)]
    source_package: Option<String>,
    /// Filter allow entries by current match status.
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
    /// Include only expired allow entries.
    #[arg(long)]
    expired: bool,
    /// Include only review-due allow entries.
    #[arg(long)]
    review_due: bool,
    /// Include only stale allow entries.
    #[arg(long)]
    stale: bool,
    /// Include only generated baseline debt entries.
    #[arg(long)]
    baseline_debt: bool,
    /// Include only entries with wildcard source-tree scopes.
    #[arg(long)]
    broad_scope: bool,
    /// Include only entries with no evidence references.
    #[arg(long)]
    missing_evidence: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ListFormat::Human)]
    format: ListFormat,
    /// Write list output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Include untracked files when determining current match status.
    #[arg(long)]
    include_untracked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ListFormat {
    Human,
    Json,
}

pub(crate) fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let parsed_filter = args.kind.as_deref().map(parse_kind_filter).transpose()?;
    let rows = list_rows(&cfg, &findings, &outcomes);
    let filters = ListFilters {
        kind: parsed_filter,
        family: args.family.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        status: args.status.as_deref(),
        expired: args.expired,
        review_due: args.review_due,
        stale: args.stale,
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        missing_evidence: args.missing_evidence,
    };
    let root_text = source_tree_root_text(&root);
    let context = ListContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        kind_arg: args.kind.as_deref(),
    };
    let text = match args.format {
        ListFormat::Human => render_list_rows(&rows, &filters),
        ListFormat::Json => render_list_rows_json(&rows, &filters, context),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
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
        inventory_source: "git_tracked",
        source_tree_root: Some("H:/Code/Rust/cargo-allow"),
        inventory_files: Some(46),
        kind_arg: Some("panic"),
    };
    render_list_rows_json(&[row], &filters, context)
}

#[cfg(test)]
#[path = "list_filter_tests.rs"]
mod filter_tests;
#[cfg(test)]
#[path = "list_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
