use allow_core::{
    AllowConfig, AllowEntry, CargoAllowResult, Finding, FindingKind, MatchOutcome, MatchStatus,
    SimpleDate,
};
use allow_match::{CheckMode, evaluate};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{
    KindFilter, RootArgs, load_world, parse_kind_filter, scope_has_wildcard, source_package_name,
    source_tree_path_matches_filter, source_tree_root_text, write_file,
};

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

#[derive(Debug, Clone)]
struct ListRow {
    id: String,
    status: MatchStatus,
    matches: usize,
    kind: FindingKind,
    family: Option<String>,
    owner: String,
    classification: String,
    scope: String,
    source_package: Option<String>,
    evidence_count: usize,
    review_after: String,
    expires: String,
    reason: String,
}

#[derive(Debug, Clone, Copy)]
struct ListFilters<'a> {
    kind: Option<KindFilter>,
    family: Option<&'a str>,
    owner: Option<&'a str>,
    classification: Option<&'a str>,
    path: Option<&'a str>,
    source_package: Option<&'a str>,
    status: Option<&'a str>,
    expired: bool,
    review_due: bool,
    stale: bool,
    baseline_debt: bool,
    broad_scope: bool,
    missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
struct ListContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
    kind_arg: Option<&'a str>,
}

impl Default for ListContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            kind_arg: None,
        }
    }
}

fn list_rows(cfg: &AllowConfig, findings: &[Finding], outcomes: &[MatchOutcome]) -> Vec<ListRow> {
    let today = SimpleDate::today_utc_approx();
    cfg.allow
        .iter()
        .map(|entry| {
            let entry_outcomes = outcomes
                .iter()
                .filter(|outcome| outcome.allow_id.as_deref() == Some(entry.id.as_str()))
                .collect::<Vec<_>>();
            ListRow {
                id: entry.id.clone(),
                status: list_entry_status(entry, &entry_outcomes, today),
                matches: entry_outcomes
                    .iter()
                    .filter(|outcome| outcome.finding_index.is_some())
                    .count(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                source_package: entry_outcomes
                    .iter()
                    .filter_map(|outcome| outcome.finding_index)
                    .filter_map(|index| findings.get(index))
                    .find_map(source_package_name),
                evidence_count: entry.evidence.len(),
                review_after: entry
                    .lifecycle
                    .review_after
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                expires: entry
                    .lifecycle
                    .expires
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                reason: entry.reason.clone(),
            }
        })
        .collect()
}

fn list_entry_status(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> MatchStatus {
    if date_is_before(entry.lifecycle.expires.as_deref(), today) {
        return MatchStatus::Expired;
    }
    if date_is_due(entry.lifecycle.review_after.as_deref(), today) {
        return MatchStatus::ReviewDue;
    }
    for status in [
        MatchStatus::New,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Stale,
    ] {
        if outcomes.iter().any(|outcome| outcome.status == status) {
            return status;
        }
    }
    if entry.classification == "baseline_debt" {
        return MatchStatus::BaselineDebt;
    }
    MatchStatus::Matched
}

fn date_is_before(date: Option<&str>, today: SimpleDate) -> bool {
    date.and_then(SimpleDate::parse)
        .map(|date| date < today)
        .unwrap_or(false)
}

fn date_is_due(date: Option<&str>, today: SimpleDate) -> bool {
    date.and_then(SimpleDate::parse)
        .map(|date| date <= today)
        .unwrap_or(false)
}

fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    let mut out = String::new();
    out.push_str("id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\treview_after\texpires\treason\n");
    let mut count = 0;
    for row in rows.iter().filter(|row| list_row_matches(row, filters)) {
        count += 1;
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.id,
            row.status.as_str(),
            row.matches,
            row.kind,
            row.family.as_deref().unwrap_or("-"),
            empty_as_dash(&row.owner),
            empty_as_dash(&row.classification),
            row.scope,
            row.source_package.as_deref().unwrap_or("-"),
            row.evidence_count,
            row.review_after,
            row.expires,
            row.reason
        ));
    }
    if count == 0 {
        out.push_str("(no allow entries matched filters)\n");
    }
    out
}

fn render_list_rows_json(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    let filtered = rows
        .iter()
        .filter(|row| list_row_matches(row, filters))
        .collect::<Vec<_>>();
    let report_rows = filtered
        .iter()
        .map(|row| allow_report::ListRow {
            id: &row.id,
            status: row.status.as_str(),
            matches: row.matches,
            kind: row.kind.as_str(),
            family: row.family.as_deref(),
            owner: &row.owner,
            classification: &row.classification,
            scope: &row.scope,
            source_package: row.source_package.as_deref(),
            evidence_count: row.evidence_count,
            review_after: dash_as_none(&row.review_after),
            expires: dash_as_none(&row.expires),
            reason: &row.reason,
        })
        .collect::<Vec<_>>();
    allow_report::render_list_json(
        &report_rows,
        allow_report::ListFilters {
            kind: context.kind_arg,
            family: filters.family,
            owner: filters.owner,
            classification: filters.classification,
            path: filters.path,
            source_package: filters.source_package,
            status: filters.status,
            expired: filters.expired,
            review_due: filters.review_due,
            stale: filters.stale,
            baseline_debt: filters.baseline_debt,
            broad_scope: filters.broad_scope,
            missing_evidence: filters.missing_evidence,
        },
        allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
    )
}

fn dash_as_none(value: &str) -> Option<&str> {
    if value == "-" { None } else { Some(value) }
}

fn list_row_matches(row: &ListRow, filters: &ListFilters<'_>) -> bool {
    if let Some(kind) = filters.kind {
        if row.kind != kind.kind || !kind.family.matches(row.family.as_deref()) {
            return false;
        }
    }
    if let Some(family) = filters.family {
        if row.family.as_deref() != Some(family) {
            return false;
        }
    }
    if let Some(owner) = filters.owner {
        if row.owner != owner {
            return false;
        }
    }
    if let Some(classification) = filters.classification {
        if row.classification != classification {
            return false;
        }
    }
    if let Some(path) = filters.path {
        if !source_tree_path_matches_filter(&row.scope, path) {
            return false;
        }
    }
    if let Some(source_package) = filters.source_package {
        if row.source_package.as_deref() != Some(source_package) {
            return false;
        }
    }
    if let Some(status) = filters.status {
        if row.status.as_str() != status {
            return false;
        }
    }
    if filters.expired && row.status != MatchStatus::Expired {
        return false;
    }
    if filters.review_due && row.status != MatchStatus::ReviewDue {
        return false;
    }
    if filters.stale && row.status != MatchStatus::Stale {
        return false;
    }
    if filters.baseline_debt && row.classification != "baseline_debt" {
        return false;
    }
    if filters.broad_scope && !scope_has_wildcard(&row.scope) {
        return false;
    }
    if filters.missing_evidence && row.evidence_count != 0 {
        return false;
    }
    true
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
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
#[path = "list_tests.rs"]
mod tests;
