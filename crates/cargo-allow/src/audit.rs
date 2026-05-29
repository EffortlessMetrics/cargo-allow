use allow_core::CargoAllowResult;
use allow_match::{CheckMode, evaluate};
use allow_policy::{broken_evidence_link_count, weak_evidence_reference_count};

#[path = "audit_args.rs"]
mod audit_args;
pub(crate) use audit_args::ReportArgs;

use crate::{
    ReportRenderArgs, load_compat_world, load_world_with_evidence_validation,
    matched_policy_missing_evidence_entries, policy_baseline_debt_entries, print_report,
    report_config,
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
        load_world_with_evidence_validation(
            args.root.root.as_deref(),
            args.config.as_deref(),
            false,
            args.kind.as_deref(),
            args.include_untracked,
            false,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    print_report(ReportRenderArgs {
        command: "audit",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        policy_missing_evidence_entries: matched_policy_missing_evidence_entries(
            &report_cfg,
            &outcomes,
        ),
        broken_evidence_links: broken_evidence_link_count(&root, &report_cfg),
        weak_evidence_references: weak_evidence_reference_count(&root, &report_cfg),
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
