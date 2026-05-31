use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use std::process;

#[path = "check_args.rs"]
mod check_args;
pub(crate) use check_args::CheckArgs;

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, ReportRenderArgs, SourceTreeReportContext,
    load_compat_world, load_world_with_evidence_mode, policy_baseline_debt_entries, print_report,
    report_config, write_file,
};

pub(crate) fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = if args.compat {
        load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?
    } else {
        load_world_with_evidence_mode(
            args.root.root.as_deref(),
            args.config.as_deref(),
            true,
            args.kind.as_deref(),
            args.include_untracked,
            EvidenceValidationMode::ReportOnly,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let mode = CheckMode::parse(
        args.mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str()),
    );
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let evidence = EvidenceReportSummary::from_policy(&root, &report_cfg, &outcomes);
    let failed =
        outcomes.iter().any(|o| mode.fails(o.status)) || evidence.has_broken_evidence_links();
    let baseline_debt_entries = policy_baseline_debt_entries(&report_cfg);
    print_report(ReportRenderArgs {
        command: "check",
        format: args.format,
        baseline_debt_entries,
        evidence,
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
            &allow_report::render_receipt_with_context_and_inventory(
                "check",
                &findings,
                &outcomes,
                failed,
                {
                    let mut context = source_context.report(Some(baseline_debt_entries));
                    evidence.apply_to(&mut context);
                    context
                },
            ),
        )?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}
