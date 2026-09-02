use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, effective_lane_posture_for_findings,
};
use allow_match::{CheckMode, evaluate};
use allow_report::{
    RECEIPT_ENFORCEMENT_ADVISORY, RECEIPT_ENFORCEMENT_ENFORCING, ReportContext, Summary,
    render_error_receipt, render_receipt_with_context_and_inventory,
};
use std::path::{Path, PathBuf};
use std::process;

#[path = "check_args.rs"]
mod check_args;
#[cfg(test)]
pub(crate) use check_args::PersistentCacheMode;
pub(crate) use check_args::{CheckArgs, CheckPhase};
#[path = "check_lane_posture.rs"]
mod check_lane_posture;
use check_lane_posture::check_failed_for_outcomes;
#[path = "check_deny.rs"]
mod check_deny;
use check_deny::{deny_escalation_failed, validate_deny_statuses};
#[path = "check_product_move_guard.rs"]
mod check_product_move_guard;
use check_product_move_guard::product_move_ledger_fails_check;
#[path = "check_extraction_shim_guard.rs"]
mod check_extraction_shim_guard;
use check_extraction_shim_guard::extraction_shim_registry_fails_check;
#[path = "check_extraction_shim_source_guard.rs"]
mod check_extraction_shim_source_guard;
use check_extraction_shim_source_guard::shim_sources_fail_check;
#[path = "check_source_coupling_guard.rs"]
mod check_source_coupling_guard;
#[path = "governance_projection.rs"]
pub(crate) mod governance_projection;
use check_source_coupling_guard::source_coupling_diagnostics_for_check;

use crate::artifact_emit;
use crate::federation_report::FederationReportBundle;
use crate::{
    EvidenceReportSummary, EvidenceValidationMode, InventoryFacts, ProfileArg, ReportRenderArgs,
    SourceTreeReportContext, assert_path_within_root, config_path, current_dir,
    evidence_inventory::current_evidence_source_tree_files, load_compat_world,
    load_read_only_world_and_cache, load_staged_world, policy_baseline_debt_entries, print_report,
    report_config, spec_precommit, spec_system, write_file,
};
use allow_inventory::{InventorySource, resolve_source_tree_root};

pub(crate) fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    cmd_check_with_persistent_cache(
        args,
        matches!(args.persistent_cache, check_args::PersistentCacheMode::On),
    )
}

pub(crate) fn cmd_check_with_persistent_cache(
    args: &CheckArgs,
    persistent_cache: bool,
) -> CargoAllowResult<()> {
    if !persistent_cache && (args.staged || args.phase.is_some() || args.staged_identity_only) {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--persistent-cache off is supported only for source-tree checks",
        ));
    }
    if args.staged || args.phase.is_some() || args.staged_identity_only {
        if args.staged_identity_only {
            return spec_precommit::cmd_staged_identity(args);
        }
        if !args.staged || !matches!(args.phase, Some(CheckPhase::Precommit)) {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                "--phase precommit requires --staged, and staged evaluation requires --phase precommit",
            ));
        }
        if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
            reject_source_exception_options(
                args.compat,
                args.kind.as_deref(),
                args.include_untracked,
                &args.deny,
            )?;
            return spec_precommit::cmd_spec_precommit(args);
        }
        if args.compat {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                "--staged source-exception evaluation does not support --compat",
            ));
        }
        if args.include_untracked {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                "--staged source-exception evaluation cannot include untracked files",
            ));
        }
        if args.tool_mode.is_some() || args.tool_digest.is_some() || args.preview_authorized {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                "--staged source-exception evaluation does not support self-hosted tool selection",
            ));
        }
        return match cmd_check_staged_source_tree(args) {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Some(path) = &args.receipt {
                    write_check_error_receipt(path, args, &err)?;
                }
                if let Some(output) = &args.output {
                    let _ = std::fs::remove_file(output);
                }
                Err(err)
            }
        };
    }
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if !persistent_cache {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                "--persistent-cache off is supported only for source-tree checks",
            ));
        }
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
            mode: args.mode.as_deref(),
        });
    }

    match cmd_check_source_tree(args, persistent_cache) {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Some(path) = &args.receipt {
                write_check_error_receipt(path, args, &err)?;
            }
            // Remove the stale --output file so CI doesn't read a prior
            // successful run's report after a failure (#2663). Best-effort:
            // if removal fails (permission, read-only FS), the error is
            // already being propagated.
            if let Some(output) = &args.output {
                let _ = std::fs::remove_file(output);
            }
            Err(err)
        }
    }
}

fn cmd_check_source_tree(args: &CheckArgs, persistent_cache: bool) -> CargoAllowResult<()> {
    // Infer format from output file extension when --format is not explicitly
    // set (#3210). Without this, `--output foo.md` silently writes human text
    // to a .md file. We only infer when the user didn't explicitly pass --format;
    // clap's default_value_t makes it impossible to distinguish "not passed"
    // from "passed as human", so we infer only when the extension strongly implies
    // a different format than the default.
    let effective_format = infer_format_from_output(args.format, args.output.as_deref());

    crate::emit_scan_status(
        "check",
        effective_format,
        args.output.as_deref(),
        args.receipt.as_deref(),
    );

    // Validate --output and --receipt paths are within the resolved source-tree
    // root (#1791). The root must be resolved from --root + cwd, not from the
    // process cwd alone: callers regularly run with an out-of-tree --root and a
    // --receipt/--output nested under that root.
    let resolved_root = resolve_check_root(args)?;
    if let Some(output) = &args.output {
        assert_path_within_root(&resolved_root, output)?;
    }
    if let Some(receipt) = &args.receipt {
        assert_path_within_root(&resolved_root, receipt)?;
    }
    let world = if args.compat {
        let (root, cfg, findings, inventory_facts) = load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?;
        crate::world::CoreWorldContext {
            root,
            cfg,
            findings,
            inventory_facts,
            federation: crate::world::default_federation_evaluation(),
        }
    } else {
        load_read_only_world_and_cache(
            args.root.root.as_deref(),
            args.config.as_deref(),
            true,
            args.kind.as_deref(),
            args.include_untracked,
            EvidenceValidationMode::ReportOnly,
            persistent_cache,
        )?
    };
    let crate::world::CoreWorldContext {
        root,
        cfg,
        findings,
        inventory_facts,
        federation,
    } = world;
    let federation_bundle = FederationReportBundle::from_evaluation(&federation);
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let mode = CheckMode::parse(
        args.mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str()),
    );
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let projected_outcomes = allow_report::ledger_project_outcomes(
        &report_cfg,
        &outcomes,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let evidence = EvidenceReportSummary::from_policy_with_source_tree_files(
        &root,
        &report_cfg,
        &outcomes,
        evidence_source_tree_files.as_ref(),
    );
    let summary = Summary::from_outcomes(&projected_outcomes);
    let baseline_debt_entries = policy_baseline_debt_entries(&report_cfg);
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let mut context = source_context.report(Some(baseline_debt_entries));
    evidence.apply_to(&mut context);
    let mirror_divergence_count = federation_bundle.mirror_divergence_advisory_count();
    if mirror_divergence_count > 0 {
        context.mirror_divergence_entries = Some(mirror_divergence_count);
    }
    let blocking_divergence_count = federation_bundle.blocking_divergence_count();
    if blocking_divergence_count > 0 {
        context.blocking_divergence_entries = Some(blocking_divergence_count);
    }
    context.rust_files_skipped = inventory_facts.rust_files_skipped;
    if !args.deny.is_empty() {
        validate_deny_statuses(&args.deny, &summary, context)?;
    }
    let product_move_ledger_failed = product_move_ledger_fails_check(&root, mode)?;
    let extraction_shim_registry_failed = extraction_shim_registry_fails_check(&root, mode)?;
    let extraction_shim_sources_failed = shim_sources_fail_check(&root, mode)?;
    let source_coupling_diagnostics = source_coupling_diagnostics_for_check(&root, mode)?;
    for diagnostic in &source_coupling_diagnostics {
        let path = allow_report::sanitize_terminal_text(&diagnostic.path.display().to_string());
        let line = allow_report::sanitize_terminal_text(&diagnostic.line.to_string());
        let column = allow_report::sanitize_terminal_text(&diagnostic.column.to_string());
        let source_owner = allow_report::sanitize_terminal_text(&diagnostic.source_owner);
        let target_crate = allow_report::sanitize_terminal_text(&diagnostic.target_crate);
        let import_text = allow_report::sanitize_terminal_text(&diagnostic.import_text);
        let relation = source_coupling_relation(diagnostic.kind);
        eprintln!(
            "source coupling: {}:{}:{}: {} {} {} ({})",
            path, line, column, source_owner, relation, target_crate, import_text,
        );
    }
    let source_coupling_failed = !source_coupling_diagnostics.is_empty();
    let failed = check_failed_for_outcomes(&outcomes, &findings, &report_cfg, mode)
        || evidence.has_broken_evidence_links()
        || federation_bundle.has_blocking_divergence()
        || (!args.deny.is_empty() && deny_escalation_failed(&args.deny, &summary, context))
        || (inventory_facts.rust_files_skipped > 0 && mode == CheckMode::NoNew)
        || (inventory_facts.rust_files_with_parse_errors > 0 && mode == CheckMode::NoNew)
        || product_move_ledger_failed
        || extraction_shim_registry_failed
        || extraction_shim_sources_failed
        || source_coupling_failed;
    if should_emit_report_stdout(
        args.output.as_deref(),
        args.receipt.as_deref(),
        effective_format,
    ) {
        print_report(ReportRenderArgs {
            command: "check",
            format: effective_format,
            baseline_debt_entries,
            evidence,
            findings: &findings,
            outcomes: &projected_outcomes,
            failed,
            output: args.output.as_deref(),
            root: &root,
            inventory_facts,
            inventory_source_identity: None,
            enforcement: Some(if mode.is_advisory() {
                RECEIPT_ENFORCEMENT_ADVISORY
            } else {
                RECEIPT_ENFORCEMENT_ENFORCING
            }),
        })?;
    } else if args.format == crate::OutputFormat::Human && args.receipt.is_some() {
        // When only --receipt is given (no --output), the full human report is
        // suppressed to keep stdout clean for CI scripts. But the operator still
        // needs a pass/fail signal — emit a brief summary to stderr (#3190).
        let status_word = if failed { "FAILED" } else { "passed" };
        eprintln!(
            "cargo-allow check: {status_word} (mode: {}, receipt written to {})",
            mode.as_str(),
            args.receipt
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new("<receipt>"))
                .display()
        );
    }
    if let Some(path) = &args.receipt {
        let policy_config = config_path(&root, args.config.as_deref())
            .map(|path| allow_report::source_tree_path_text(&path));
        let effective_mode = args
            .mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str());
        let lane_posture = effective_lane_posture_for_findings(
            &report_cfg.lanes,
            findings.iter().map(|finding| finding.kind),
        );
        let provenance = run_provenance();
        let policy_digest = inventory_facts.policy_digest_text();
        let bindings = receipt_provenance_bindings(&root, policy_digest.as_deref());
        let receipt = federation_bundle.with_context(|federation_context| {
            let mut receipt_context = source_context.report(Some(baseline_debt_entries));
            evidence.apply_to(&mut receipt_context);
            if mirror_divergence_count > 0 {
                receipt_context.mirror_divergence_entries = Some(mirror_divergence_count);
            }
            if blocking_divergence_count > 0 {
                receipt_context.blocking_divergence_entries = Some(blocking_divergence_count);
            }
            apply_receipt_run_metadata(
                &mut receipt_context,
                effective_mode,
                mode,
                policy_config.as_deref(),
                &provenance.started_at,
                &provenance.run_id,
                &bindings,
            );
            receipt_context.lane_posture = Some(&lane_posture);
            receipt_context.federation = Some(federation_context);
            render_receipt_with_context_and_inventory(
                "check",
                &findings,
                &projected_outcomes,
                failed,
                receipt_context,
            )
        });
        write_file(path, &receipt)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    }
    if let (Some(artifact_dir), Some(emit_raw)) = (&args.artifact_dir, &args.emit) {
        let formats = match artifact_emit::parse_emit_formats(emit_raw) {
            Ok(formats) => formats,
            Err(error) => {
                eprintln!("cargo-allow check: {error}");
                process::exit(1);
            }
        };
        let mut artifact_context = source_context.report(Some(baseline_debt_entries));
        evidence.apply_to(&mut artifact_context);
        artifact_context.mode = Some(mode.as_str());
        let policy_config_text = config_path(&root, args.config.as_deref())
            .map(|path| allow_report::source_tree_path_text(&path));
        artifact_context.policy_config = policy_config_text.as_deref();
        artifact_context.tool_version = Some(env!("CARGO_PKG_VERSION"));
        let emit_ctx = artifact_emit::ArtifactEmitContext {
            command: "check",
            findings: &findings,
            outcomes: &projected_outcomes,
            failed,
            report_context: &artifact_context,
            receipt_context: None,
        };
        let source_subj = format!("worktree:{}", mode.as_str());
        let result_class = if failed {
            allow_report::EvaluationResultClassV2::Blocking
        } else {
            allow_report::EvaluationResultClassV2::Passed
        };
        if let Err(error) = artifact_emit::emit_artifact_set(
            artifact_dir,
            &artifact_emit::EmitConfig {
                operation: "check",
                formats: &formats,
                result_class,
                blocking: failed,
                resolved_config_identity: &inventory_facts.policy_digest_text().unwrap_or_default(),
                source_subject: &source_subj,
            },
            &emit_ctx,
        ) {
            eprintln!("cargo-allow check: artifact emit: {error}");
            process::exit(1);
        }
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

pub(crate) fn source_coupling_relation(
    kind: check_source_coupling_guard::SourceCouplingDiagnosticKind,
) -> &'static str {
    match kind {
        check_source_coupling_guard::SourceCouplingDiagnosticKind::Import => "imports",
        check_source_coupling_guard::SourceCouplingDiagnosticKind::PathRead => "reads",
        check_source_coupling_guard::SourceCouplingDiagnosticKind::IntegrationTestDependency => {
            "uses integration-test dependency"
        }
    }
}

fn cmd_check_staged_source_tree(args: &CheckArgs) -> CargoAllowResult<()> {
    crate::emit_scan_status(
        "check",
        args.format,
        args.output.as_deref(),
        args.receipt.as_deref(),
    );
    let resolved_root = resolve_check_root(args)?;
    if let Some(output) = &args.output {
        assert_path_within_root(&resolved_root, output)?;
    }
    if let Some(receipt) = &args.receipt {
        assert_path_within_root(&resolved_root, receipt)?;
    }
    let staged = load_staged_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        args.kind.as_deref(),
    )?;
    if let Some(expected) = args.expect_staged_identity.as_deref()
        && expected != staged.source_identity
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "staged identity did not match --expect-staged-identity: expected {expected}, observed {}",
                staged.source_identity
            ),
        ));
    }
    let report_cfg = report_config(&staged.cfg, args.kind.as_deref())?;
    let mode = CheckMode::parse(
        args.mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str()),
    );
    if staged.product_move_ledger_present && matches!(mode, CheckMode::NoNew | CheckMode::Strict) {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Unsupported,
            "exact staged source-exception evaluation does not yet support product-move ledger enforcement in no-new or strict mode; use audit mode or the tracked-worktree check",
        ));
    }
    let outcomes = evaluate(&report_cfg, &staged.findings, mode);
    let projected_outcomes = allow_report::ledger_project_outcomes(
        &report_cfg,
        &outcomes,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let evidence = EvidenceReportSummary::from_policy_with_source_tree_files(
        &staged.root,
        &report_cfg,
        &outcomes,
        Some(&staged.evidence_source_tree_files),
    );
    let summary = Summary::from_outcomes(&projected_outcomes);
    let baseline_debt_entries = policy_baseline_debt_entries(&report_cfg);
    let source_context = SourceTreeReportContext::new_with_identity(
        &staged.root,
        staged.inventory_facts,
        Some(&staged.source_identity),
    );
    let mut context = source_context.report(Some(baseline_debt_entries));
    evidence.apply_to(&mut context);
    let federation_bundle = FederationReportBundle::from_evaluation(&staged.federation);
    let mirror_divergence_count = federation_bundle.mirror_divergence_advisory_count();
    if mirror_divergence_count > 0 {
        context.mirror_divergence_entries = Some(mirror_divergence_count);
    }
    let blocking_divergence_count = federation_bundle.blocking_divergence_count();
    if blocking_divergence_count > 0 {
        context.blocking_divergence_entries = Some(blocking_divergence_count);
    }
    context.rust_files_skipped = staged.inventory_facts.rust_files_skipped;
    if !args.deny.is_empty() {
        validate_deny_statuses(&args.deny, &summary, context)?;
    }
    let failed = check_failed_for_outcomes(&outcomes, &staged.findings, &report_cfg, mode)
        || evidence.has_broken_evidence_links()
        || federation_bundle.has_blocking_divergence()
        || (!args.deny.is_empty() && deny_escalation_failed(&args.deny, &summary, context))
        || (staged.inventory_facts.completeness == allow_inventory::InventoryCompleteness::Partial
            && mode == CheckMode::NoNew)
        || (staged.inventory_facts.rust_files_with_parse_errors > 0 && mode == CheckMode::NoNew);
    if should_emit_report_stdout(args.output.as_deref(), args.receipt.as_deref(), args.format) {
        print_report(ReportRenderArgs {
            command: "check",
            format: args.format,
            baseline_debt_entries,
            evidence,
            findings: &staged.findings,
            outcomes: &projected_outcomes,
            failed,
            output: args.output.as_deref(),
            root: &staged.root,
            inventory_facts: staged.inventory_facts,
            inventory_source_identity: Some(&staged.source_identity),
            enforcement: Some(if mode.is_advisory() {
                RECEIPT_ENFORCEMENT_ADVISORY
            } else {
                RECEIPT_ENFORCEMENT_ENFORCING
            }),
        })?;
    }
    if let Some(path) = &args.receipt {
        let policy_config = config_path(&staged.root, args.config.as_deref())
            .map(|path| allow_core::strip_win32_verbatim_prefix(&path.display().to_string()));
        let effective_mode = args
            .mode
            .as_deref()
            .unwrap_or(report_cfg.workspace.default_mode.as_str());
        let lane_posture = effective_lane_posture_for_findings(
            &report_cfg.lanes,
            staged.findings.iter().map(|finding| finding.kind),
        );
        let provenance = run_provenance();
        let policy_digest = staged.inventory_facts.policy_digest_text();
        let bindings = receipt_provenance_bindings(&staged.root, policy_digest.as_deref());
        let receipt = federation_bundle.with_context(|federation_context| {
            let mut receipt_context = source_context.report(Some(baseline_debt_entries));
            evidence.apply_to(&mut receipt_context);
            if mirror_divergence_count > 0 {
                receipt_context.mirror_divergence_entries = Some(mirror_divergence_count);
            }
            if blocking_divergence_count > 0 {
                receipt_context.blocking_divergence_entries = Some(blocking_divergence_count);
            }
            apply_receipt_run_metadata(
                &mut receipt_context,
                effective_mode,
                mode,
                policy_config.as_deref(),
                &provenance.started_at,
                &provenance.run_id,
                &bindings,
            );
            receipt_context.lane_posture = Some(&lane_posture);
            receipt_context.federation = Some(federation_context);
            render_receipt_with_context_and_inventory(
                "check",
                &staged.findings,
                &projected_outcomes,
                failed,
                receipt_context,
            )
        });
        write_file(path, &receipt)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    }
    if failed {
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
    // Never write an error receipt outside the source-tree root: if the run
    // failed because --receipt escaped the root, writing the error receipt
    // there would itself violate the containment contract (#1791).
    assert_path_within_root(&root, path)?;
    let mode = args
        .mode
        .as_deref()
        .map(CheckMode::parse)
        .unwrap_or(CheckMode::NoNew);
    let default_cfg = allow_core::AllowConfig::empty();
    let policy_config = config_path(&root, args.config.as_deref())
        .map(|path| allow_core::strip_win32_verbatim_prefix(&path.display().to_string()));
    let effective_mode = args
        .mode
        .as_deref()
        .unwrap_or(default_cfg.workspace.default_mode.as_str());
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::source_only(InventorySource::FilesystemFallback),
    );
    let mut context = source_context.report(None);
    let provenance = run_provenance();
    let bindings = receipt_provenance_bindings(&root, None);
    apply_receipt_run_metadata(
        &mut context,
        effective_mode,
        mode,
        policy_config.as_deref(),
        &provenance.started_at,
        &provenance.run_id,
        &bindings,
    );
    write_file(path, &render_error_receipt(&err.to_string(), context))
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)
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

/// Infer the output format from the --output file extension when the user
/// didn't explicitly pass --format (#3210). Without this, `--output foo.md`
/// silently writes human text to a .md file.
fn infer_format_from_output(
    declared: crate::OutputFormat,
    output: Option<&Path>,
) -> crate::OutputFormat {
    // Only infer when the declared format is the clap default (Human).
    // If the user explicitly passed --format json, respect that.
    if declared != crate::OutputFormat::Human {
        return declared;
    }
    let Some(ext) = output.and_then(|p| p.extension()).and_then(|e| e.to_str()) else {
        return declared;
    };
    match ext.to_ascii_lowercase().as_str() {
        "json" => crate::OutputFormat::Json,
        "md" => crate::OutputFormat::Markdown,
        "html" | "htm" => crate::OutputFormat::Html,
        "sarif" => crate::OutputFormat::Sarif,
        _ => declared,
    }
}

fn apply_receipt_run_metadata<'a>(
    context: &mut ReportContext<'a>,
    effective_mode: &'a str,
    mode: CheckMode,
    policy_config: Option<&'a str>,
    started_at: &'a str,
    run_id: &'a str,
    bindings: &'a ReceiptProvenanceBindings,
) {
    context.mode = Some(effective_mode);
    context.enforcement = Some(if mode.is_advisory() {
        RECEIPT_ENFORCEMENT_ADVISORY
    } else {
        RECEIPT_ENFORCEMENT_ENFORCING
    });
    context.policy_config = policy_config;
    context.tool_version = Some(env!("CARGO_PKG_VERSION"));
    // Run provenance (#1854): started_at + run_id so a consumer can correlate a
    // receipt to a specific CI run / wall-clock time. Receipts with timestamps
    // are NOT byte-stable across runs (documented).
    context.started_at = Some(started_at);
    context.run_id = Some(run_id);
    // Integrity binding (#1850/#1781): retain the contextual HEAD and exact
    // evaluated policy bytes so an external verifier can detect mismatches.
    // These unsigned fields do not authenticate the receipt, invocation, or
    // source bytes.
    context.git_sha = bindings.git_sha.as_deref();
    context.policy_digest = bindings.policy_digest.as_deref();
}

/// Best-effort receipt integrity binding (#1850/#1781).
///
/// `git_sha` resolves `HEAD` inside `root`; any failure (not a repository, git
/// missing, detached/empty state) leaves it absent rather than failing the scan.
/// `policy_digest` is the versioned SHA-256 of the exact active ledger bytes
/// loaded and evaluated for a successful check. Generic error receipts omit it
/// because the error writer does not retain evaluated provenance; absence does
/// not prove that evaluation never began.
struct ReceiptProvenanceBindings {
    git_sha: Option<String>,
    policy_digest: Option<String>,
}

fn receipt_provenance_bindings(
    root: &Path,
    policy_digest: Option<&str>,
) -> ReceiptProvenanceBindings {
    ReceiptProvenanceBindings {
        git_sha: best_effort_head_commit(root),
        policy_digest: policy_digest.map(str::to_owned),
    }
}

fn best_effort_head_commit(root: &Path) -> Option<String> {
    let output = process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty() || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(commit)
}

/// Run provenance: a UTC timestamp + a process-unique run id (#1854).
struct RunProvenance {
    started_at: String,
    run_id: String,
}

fn run_provenance() -> RunProvenance {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // RFC 3339 UTC timestamp from epoch seconds (no chrono dependency).
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let date = allow_core::SimpleDate::from_days_since_unix_epoch(days);
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    let started_at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.year, date.month, date.day, h, m, s
    );
    let run_id = format!("cargo-allow-{}-{}", std::process::id(), secs);
    RunProvenance { started_at, run_id }
}

fn resolve_check_root(args: &CheckArgs) -> CargoAllowResult<PathBuf> {
    let cwd = current_dir()?;
    resolve_source_tree_root(args.root.root.as_deref(), cwd)
}

fn reject_source_exception_options(
    compat: bool,
    kind: Option<&str>,
    include_untracked: bool,
    deny: &[String],
) -> CargoAllowResult<()> {
    if compat {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--compat is not supported with --profile spec-system; remove --compat or drop --profile spec-system",
        ));
    }
    if kind.is_some() {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--kind is not supported with --profile spec-system; remove --kind or drop --profile spec-system",
        ));
    }
    if include_untracked {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--include-untracked is not supported with --profile spec-system; remove --include-untracked or drop --profile spec-system",
        ));
    }
    if !deny.is_empty() {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--deny is not supported with --profile spec-system; remove --deny or drop --profile spec-system",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "check_receipt_tests.rs"]
mod receipt_tests;

#[cfg(test)]
mod provenance_tests {
    use super::run_provenance;

    #[test]
    fn run_provenance_emits_rfc3339_utc_timestamp() {
        let provenance = run_provenance();
        let ts = &provenance.started_at;

        // Must end with Z (UTC)
        assert!(ts.ends_with('Z'), "timestamp should end with Z: {ts}");

        // Must have the structure YYYY-MM-DDTHH:MM:SSZ (20 chars)
        assert_eq!(
            ts.len(),
            20,
            "timestamp should be 20 chars (YYYY-MM-DDTHH:MM:SSZ): {ts}"
        );
        assert_eq!(
            ts.chars().nth(4),
            Some('-'),
            "position 4 should be dash: {ts}"
        );
        assert_eq!(
            ts.chars().nth(7),
            Some('-'),
            "position 7 should be dash: {ts}"
        );
        assert_eq!(
            ts.chars().nth(10),
            Some('T'),
            "position 10 should be T: {ts}"
        );
        assert_eq!(
            ts.chars().nth(13),
            Some(':'),
            "position 13 should be colon: {ts}"
        );
        assert_eq!(
            ts.chars().nth(16),
            Some(':'),
            "position 16 should be colon: {ts}"
        );

        // Year should be 2020+ (sanity check against epoch=0 regression)
        let year: u32 = ts.get(..4).and_then(|s| s.parse().ok()).unwrap_or(0);
        assert!(
            year >= 2020,
            "timestamp year should be 2020+: {ts} (got {year})"
        );

        // run_id should be non-empty and contain the process ID
        assert!(!provenance.run_id.is_empty(), "run_id should be non-empty");
        assert!(
            provenance.run_id.starts_with("cargo-allow-"),
            "run_id should start with cargo-allow-: {}",
            provenance.run_id
        );
    }

    #[test]
    fn run_provenance_components_are_valid() {
        let provenance = run_provenance();
        let ts = &provenance.started_at;
        let parts: Vec<&str> = ts.trim_end_matches('Z').split('T').collect();
        assert_eq!(parts.len(), 2, "timestamp should have date and time parts");

        let date_parts: Vec<&str> = parts.first().unwrap_or(&"").split('-').collect();
        assert_eq!(date_parts.len(), 3, "date should have YYYY-MM-DD");
        let month: u32 = date_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let day: u32 = date_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        assert!((1..=12).contains(&month), "month should be 1-12: {month}");
        assert!((1..=31).contains(&day), "day should be 1-31: {day}");

        let time_parts: Vec<&str> = parts.get(1).unwrap_or(&"").split(':').collect();
        assert_eq!(time_parts.len(), 3, "time should have HH:MM:SS");
        let h: u32 = time_parts
            .first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(99);
        let m: u32 = time_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(99);
        let s: u32 = time_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(99);
        assert!(h < 24, "hour should be 0-23: {h}");
        assert!(m < 60, "minute should be 0-59: {m}");
        assert!(s < 60, "second should be 0-59: {s}");
    }
}
