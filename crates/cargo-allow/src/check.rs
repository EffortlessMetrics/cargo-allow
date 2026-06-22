use allow_core::{
    CargoAllowError, CargoAllowResult, MatchStatus, effective_lane_posture_for_findings,
};
use allow_match::{CheckMode, evaluate};
use allow_report::{
    RECEIPT_ENFORCEMENT_ADVISORY, RECEIPT_ENFORCEMENT_ENFORCING, ReportContext, Summary,
    render_error_receipt, render_receipt_with_context_and_inventory,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process;

#[path = "check_args.rs"]
mod check_args;
pub(crate) use check_args::CheckArgs;
#[path = "check_lane_posture.rs"]
mod check_lane_posture;
use check_lane_posture::check_failed_for_outcomes;
#[path = "check_deny.rs"]
mod check_deny;
use check_deny::{deny_escalation_failed, validate_deny_statuses};

use crate::federation_report::FederationReportBundle;
use crate::{
    EvidenceReportSummary, EvidenceValidationMode, InventoryFacts, ProfileArg, ReportRenderArgs,
    SourceTreeReportContext, config_path, evidence_inventory::current_evidence_source_tree_files,
    load_compat_world, load_world_with_evidence_mode, policy_baseline_debt_entries, print_report,
    report_config, spec_system, write_file,
};
use allow_inventory::{InventorySource, resolve_source_tree_root};

pub(crate) fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        reject_source_exception_options(
            args.compat,
            args.kind.as_deref(),
            args.include_untracked,
            &args.deny,
        )?;
        return spec_system::cmd_spec_system(spec_system::SpecSystemCommandArgs {
            command: "check",
            root: &args.root,
            config: args.config.as_deref(),
            format: args.format,
            output: args.output.as_deref(),
            receipt: args.receipt.as_deref(),
        });
    }

    match cmd_check_source_tree(args) {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Some(path) = &args.receipt {
                write_check_error_receipt(path, args, &err)?;
            }
            Err(err)
        }
    }
}

fn cmd_check_source_tree(args: &CheckArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts, federation) = if args.compat {
        let (root, cfg, findings, inventory_facts) = load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?;
        (
            root,
            cfg,
            findings,
            inventory_facts,
            crate::world::default_federation_evaluation(),
        )
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
    let federation_bundle = FederationReportBundle::from_evaluation(&federation);
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let mode = CheckMode::parse(
        args.mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str()),
    );
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let evidence = EvidenceReportSummary::from_policy_with_source_tree_files(
        &root,
        &report_cfg,
        &outcomes,
        evidence_source_tree_files.as_ref(),
    );
    let summary = Summary::from_outcomes(&outcomes);
    let baseline_debt_entries = policy_baseline_debt_entries(&report_cfg);
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let mut context = source_context.report(Some(baseline_debt_entries));
    evidence.apply_to(&mut context);
    let mirror_divergence_count = federation_bundle.mirror_divergence_advisory_count();
    if mirror_divergence_count > 0 {
        context.mirror_divergence_entries = Some(mirror_divergence_count);
    }
    if !args.deny.is_empty() {
        validate_deny_statuses(&args.deny)?;
    }
    let failed = check_failed_for_outcomes(&outcomes, &findings, &report_cfg, mode)
        || evidence.has_broken_evidence_links()
        || federation_bundle.has_blocking_divergence()
        || (!args.deny.is_empty() && deny_escalation_failed(&args.deny, &summary, context));
    if should_emit_report_stdout(args.output.as_deref(), args.receipt.as_deref(), args.format) {
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
    }
    if let Some(path) = &args.receipt {
        let policy_config =
            config_path(&root, args.config.as_deref()).map(|path| path.display().to_string());
        let effective_mode = args
            .mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str());
        let lane_posture = effective_lane_posture_for_findings(
            &report_cfg.lanes,
            findings.iter().map(|finding| finding.kind),
        );
        let receipt = federation_bundle.with_context(|federation_context| {
            let mut receipt_context = source_context.report(Some(baseline_debt_entries));
            evidence.apply_to(&mut receipt_context);
            if mirror_divergence_count > 0 {
                receipt_context.mirror_divergence_entries = Some(mirror_divergence_count);
            }
            apply_receipt_run_metadata(
                &mut receipt_context,
                effective_mode,
                mode,
                policy_config.as_deref(),
            );
            receipt_context.lane_posture = Some(&lane_posture);
            receipt_context.federation = Some(federation_context);
            render_receipt_with_context_and_inventory(
                "check",
                &findings,
                &outcomes,
                failed,
                receipt_context,
            )
        });
        write_file(path, &receipt)?;
    }
    if failed {
        // When --receipt is set and format is Human, the report is suppressed
        // from stdout. Print a one-line summary to stderr so the user knows
        // WHY the check failed without opening the JSON receipt (#1816).
        if args.receipt.is_some()
            && !should_emit_report_stdout(
                args.output.as_deref(),
                args.receipt.as_deref(),
                args.format,
            )
        {
            let new_count = outcomes
                .iter()
                .filter(|o| o.status == MatchStatus::New)
                .count();
            let expired_count = outcomes
                .iter()
                .filter(|o| o.status == MatchStatus::Expired)
                .count();
            eprintln!("check failed: {new_count} new finding(s), {expired_count} expired entries");
        }
        process::exit(1);
    }
    Ok(())
}

fn write_check_error_receipt(
    path: &Path,
    args: &CheckArgs,
    err: &CargoAllowError,
) -> CargoAllowResult<()> {
    let root = resolve_check_root(args)?;
    let mode = args
        .mode
        .as_deref()
        .map(CheckMode::parse)
        .unwrap_or(CheckMode::NoNew);
    let default_cfg = allow_core::AllowConfig::empty();
    let policy_config =
        config_path(&root, args.config.as_deref()).map(|path| path.display().to_string());
    let effective_mode = args
        .mode
        .as_deref()
        .unwrap_or(default_cfg.workspace.default_mode.as_str());
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::source_only(InventorySource::FilesystemFallback),
    );
    let mut context = source_context.report(None);
    apply_receipt_run_metadata(&mut context, effective_mode, mode, policy_config.as_deref());
    write_file(path, &render_error_receipt(&err.to_string(), context))
}

fn should_emit_report_stdout(
    output: Option<&Path>,
    receipt: Option<&Path>,
    format: crate::OutputFormat,
) -> bool {
    if output.is_some() {
        return true;
    }
    !(receipt.is_some() && format == crate::OutputFormat::Human)
}

fn apply_receipt_run_metadata<'a>(
    context: &mut ReportContext<'a>,
    effective_mode: &'a str,
    mode: CheckMode,
    policy_config: Option<&'a str>,
) {
    context.mode = Some(effective_mode);
    context.enforcement = Some(if mode.is_advisory() {
        RECEIPT_ENFORCEMENT_ADVISORY
    } else {
        RECEIPT_ENFORCEMENT_ENFORCING
    });
    context.policy_config = policy_config;
    context.tool_version = Some(env!("CARGO_PKG_VERSION"));
}

fn resolve_check_root(args: &CheckArgs) -> CargoAllowResult<PathBuf> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    resolve_source_tree_root(args.root.root.as_deref(), cwd)
}

fn reject_source_exception_options(
    compat: bool,
    kind: Option<&str>,
    include_untracked: bool,
    deny: &[String],
) -> CargoAllowResult<()> {
    if compat {
        return Err(CargoAllowError::new(
            "--compat is not supported with --profile spec-system",
        ));
    }
    if kind.is_some() {
        return Err(CargoAllowError::new(
            "--kind is not supported with --profile spec-system",
        ));
    }
    if include_untracked {
        return Err(CargoAllowError::new(
            "--include-untracked is not supported with --profile spec-system",
        ));
    }
    if !deny.is_empty() {
        return Err(CargoAllowError::new(
            "--deny is not supported with --profile spec-system",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "check_receipt_tests.rs"]
mod receipt_tests;
