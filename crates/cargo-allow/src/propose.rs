use allow_core::{CargoAllowResult, MatchStatus};
use allow_match::{CheckMode, evaluate};
use allow_policy::render_policy;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, load_world, write_file, write_file_no_overwrite};

#[path = "propose_baseline.rs"]
mod propose_baseline;
#[path = "propose_render.rs"]
mod propose_render;
#[path = "propose_types.rs"]
mod propose_types;
use propose_baseline::{default_baseline_expiry, entry_from_finding};
use propose_render::{render_propose_summary, render_propose_summary_json};
pub(super) use propose_types::ProposeContext;

#[cfg(test)]
use allow_core::{Finding, FindingKind, SimpleDate};
#[cfg(test)]
use propose_baseline::BASELINE_DEBT_DEFAULT_DAYS;

#[derive(Debug, Clone, Parser)]
pub(crate) struct ProposeArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Expiry date for generated baseline_debt entries. Defaults to 67 days from today.
    #[arg(long)]
    expires: Option<String>,
    /// Write proposed policy to this path.
    #[arg(long)]
    write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = ProposeSummaryFormat::Human)]
    summary_format: ProposeSummaryFormat,
    /// Write proposal summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ProposeSummaryFormat {
    Human,
    Json,
}

pub(crate) fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    let mut proposed_entries = 0;
    let expires = args.expires.clone().unwrap_or_else(default_baseline_expiry);
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == MatchStatus::New)
        .enumerate()
    {
        if let Some(finding) = outcome.finding_index.and_then(|idx| findings.get(idx)) {
            proposed
                .allow
                .push(entry_from_finding(finding, start + n, &expires));
            proposed_entries += 1;
        }
    }
    let rendered = render_policy(&proposed);
    if let Some(path) = &args.write {
        write_file_no_overwrite(path, &rendered, args.force)?;
    } else {
        println!("{rendered}");
    }
    let root_text = allow_report::source_tree_path_text(&root);
    let context = ProposeContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        kind_filter: args.kind.as_deref(),
    };
    let summary = match args.summary_format {
        ProposeSummaryFormat::Human => render_propose_summary(
            findings.len(),
            proposed_entries,
            expires.as_str(),
            args.write.as_deref(),
        ),
        ProposeSummaryFormat::Json => render_propose_summary_json(
            findings.len(),
            proposed_entries,
            expires.as_str(),
            args.write.as_deref(),
            args.force,
            context,
        ),
    };
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_propose_json_for_contract_test() -> String {
    use std::path::Path;

    render_propose_summary_json(
        12,
        3,
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
        true,
        ProposeContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(51),
            kind_filter: Some("panic"),
        },
    )
}

#[cfg(test)]
#[path = "propose_tests.rs"]
mod tests;
