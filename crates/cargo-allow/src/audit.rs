use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};

#[path = "audit_args.rs"]
mod audit_args;
pub(crate) use audit_args::ReportArgs;

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, ReportRenderArgs, load_compat_world,
    load_world_with_evidence_mode, policy_baseline_debt_entries, print_report, report_config,
};

pub(crate) fn cmd_audit(args: &ReportArgs) -> CargoAllowResult<()> {
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
            false,
            args.kind.as_deref(),
            args.include_untracked,
            EvidenceValidationMode::ReportOnly,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    let evidence = EvidenceReportSummary::from_policy(&root, &report_cfg, &outcomes);
    print_report(ReportRenderArgs {
        command: "audit",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        evidence,
        findings: &findings,
        outcomes: &outcomes,
        failed: false,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
