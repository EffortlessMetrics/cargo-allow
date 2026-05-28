use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use clap::Parser;
use std::path::PathBuf;
use std::process;

use crate::{
    OutputFormat, ReportRenderArgs, RootArgs, SourceTreeReportContext, load_compat_world,
    load_world, policy_baseline_debt_entries, print_report, report_config, write_file,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct CheckArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    pub(crate) compat: bool,
    /// Filter findings by kind.
    #[arg(long)]
    pub(crate) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(crate) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Write machine-readable receipt to a file.
    #[arg(long)]
    pub(crate) receipt: Option<PathBuf>,
    /// Check mode.
    #[arg(long, default_value = "no-new", value_parser = ["audit", "no-new", "strict", "release"])]
    pub(crate) mode: String,
}

pub(crate) fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    let mode = CheckMode::parse(&args.mode);
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
            true,
            args.kind.as_deref(),
            args.include_untracked,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let failed = outcomes.iter().any(|o| mode.fails(o.status));
    print_report(ReportRenderArgs {
        command: "check",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        findings: &findings,
        outcomes: &outcomes,
        failed,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
    })?;
    if let Some(path) = &args.receipt {
        let source_context = SourceTreeReportContext::new(&root, inventory_facts);
        write_file(
            path,
            &allow_report::render_receipt_with_context(
                "check",
                &outcomes,
                failed,
                source_context.report(None),
            ),
        )?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}
