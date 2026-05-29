use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use allow_policy::broken_evidence_link_count;
use std::process;

#[path = "check_args.rs"]
mod check_args;
pub(crate) use check_args::CheckArgs;

use crate::{
    ReportRenderArgs, SourceTreeReportContext, load_compat_world,
    load_world_with_evidence_validation, policy_baseline_debt_entries, print_report, report_config,
    write_file,
};

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
        load_world_with_evidence_validation(
            args.root.root.as_deref(),
            args.config.as_deref(),
            true,
            args.kind.as_deref(),
            args.include_untracked,
            false,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let broken_evidence_links = broken_evidence_link_count(&root, &report_cfg);
    let failed = outcomes.iter().any(|o| mode.fails(o.status)) || broken_evidence_links > 0;
    let baseline_debt_entries = policy_baseline_debt_entries(&report_cfg);
    print_report(ReportRenderArgs {
        command: "check",
        format: args.format,
        baseline_debt_entries,
        broken_evidence_links,
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
            &allow_report::render_receipt_with_context("check", &outcomes, failed, {
                let mut context = source_context.report(Some(baseline_debt_entries));
                context.broken_evidence_links =
                    (broken_evidence_links > 0).then_some(broken_evidence_links);
                context
            }),
        )?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}
