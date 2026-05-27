use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use clap::Parser;
use std::path::PathBuf;

use crate::{
    OutputFormat, ReportRenderArgs, RootArgs, load_compat_world, load_world,
    policy_baseline_debt_entries, print_report, report_config,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReportArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    compat: bool,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

pub(crate) fn cmd_audit(args: &ReportArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = if args.compat {
        load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?
    } else {
        load_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            false,
            args.kind.as_deref(),
            args.include_untracked,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    print_report(ReportRenderArgs {
        command: "audit",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        findings: &findings,
        outcomes: &outcomes,
        failed: false,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
    })?;
    eprintln!("source tree: {}", root.display());
    Ok(())
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
