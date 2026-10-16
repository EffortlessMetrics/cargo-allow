use crate::artifact_emit;
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_match::{CheckMode, evaluate};

#[path = "audit_args.rs"]
mod audit_args;
pub(crate) use audit_args::ReportArgs;

use crate::{
    EvidenceReportSummary, EvidenceValidationMode, ProfileArg, ReportRenderArgs,
    evidence_inventory::current_evidence_source_tree_files, load_compat_world,
    load_read_only_world, policy_baseline_debt_entries, print_report, report_config,
    reporting::SourceTreeReportContext, spec_system,
};

pub(crate) fn cmd_audit(args: &ReportArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        reject_source_exception_options(args.compat, args.kind.as_deref(), args.include_untracked)?;
        // #4547-class: the spec-system path never reads these flags; fail
        // honest instead of silently ignoring them behind a green exit.
        if args.emit.is_some() || args.artifact_dir.is_some() {
            return Err(CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Unsupported,
                "audit --profile spec-system does not support multi-format artifact emission; \
                 drop --emit and --artifact-dir, then retry",
            ));
        }
        return spec_system::cmd_spec_system(spec_system::SpecSystemCommandArgs {
            command: "audit",
            root: &args.root,
            config: args.config.as_deref(),
            format: args.format,
            output: args.output.as_deref(),
            receipt: None,
            // `audit` is report-only and exposes no `--mode`.
            mode: None,
        });
    }

    crate::emit_scan_status("audit", args.format, args.output.as_deref(), None);

    let world = if args.compat {
        load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )
        .map(
            |(root, cfg, findings, inventory_facts)| crate::world::CoreWorldContext {
                root,
                cfg,
                findings,
                inventory_facts,
                federation: crate::world::default_federation_evaluation(),
            },
        )?
    } else {
        load_read_only_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            false,
            args.kind.as_deref(),
            args.include_untracked,
            EvidenceValidationMode::ReportOnly,
        )?
    };
    let crate::world::CoreWorldContext {
        root,
        cfg,
        findings,
        inventory_facts,
        federation: _federation,
    } = world;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let as_of = allow_core::SimpleDate::today_utc_approx();
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit, as_of);
    let projected_outcomes = allow_report::ledger_project_outcomes(&report_cfg, &outcomes, as_of);
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    let evidence = EvidenceReportSummary::from_policy_with_source_tree_files(
        &root,
        &report_cfg,
        &outcomes,
        evidence_source_tree_files.as_ref(),
    );
    print_report(ReportRenderArgs {
        command: "audit",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        evidence,
        findings: &findings,
        outcomes: &projected_outcomes,
        failed: false,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
        inventory_source_identity: None,
        // `audit` never fails a run; its pass is advisory by definition.
        enforcement: Some(allow_report::RECEIPT_ENFORCEMENT_ADVISORY),
        artifact_emission: args.artifact_dir.as_deref(),
    })?;

    if let (Some(artifact_dir), Some(emit_raw)) = (&args.artifact_dir, &args.emit) {
        // The identical mistake on `check` exits 2 with a typed Usage error
        // (#4505): route bad --emit values through the same typed path.
        let formats = artifact_emit::parse_emit_formats(emit_raw).map_err(|error| {
            CargoAllowError::with_kind(allow_core::CargoAllowErrorKind::Usage, error)
        })?;
        let source_ctx = SourceTreeReportContext::new(&root, inventory_facts);
        let mut artifact_context =
            source_ctx.report(Some(policy_baseline_debt_entries(&report_cfg)));
        evidence.apply_to(&mut artifact_context);
        artifact_context.tool_version = Some(env!("CARGO_PKG_VERSION"));
        let emit_ctx = artifact_emit::ArtifactEmitContext {
            command: "audit",
            findings: &findings,
            outcomes: &projected_outcomes,
            failed: false,
            report_context: &artifact_context,
            receipt_context: None,
        };
        let source_subj = format!("audit:{}", args.kind.as_deref().unwrap_or("all"));
        let artifacts = artifact_emit::emit_artifact_set(
            artifact_dir,
            &artifact_emit::EmitConfig {
                operation: "audit",
                formats: &formats,
                result_class: allow_report::EvaluationResultClassV2::Advisory,
                blocking: false,
                resolved_config_identity: &inventory_facts.policy_digest_text().unwrap_or_default(),
                source_subject: &source_subj,
            },
            &emit_ctx,
        )
        .map_err(|error| {
            CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Artifact,
                format!("artifact emit: {error}"),
            )
        })?;
        // Validate the emitted set like check does (#4526): a parser-accepted
        // format whose render fails (for example `--emit human`) must fail
        // the invocation, not exit 0 with a RenderFailed manifest.
        let validation = artifacts.validate();
        if !matches!(
            validation.result,
            allow_report::EvaluationArtifactSetResultV2::Complete
                | allow_report::EvaluationArtifactSetResultV2::SemanticNonGreen
        ) {
            let details = validation
                .gaps
                .into_iter()
                .chain(
                    artifacts
                        .artifacts
                        .iter()
                        .flat_map(|artifact| artifact.render_errors.iter().cloned()),
                )
                .collect::<Vec<_>>();
            let error = CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Artifact,
                format!(
                    "artifact emit {:?}: {}",
                    validation.result,
                    details.join("; ")
                ),
            );
            if artifacts
                .artifacts
                .iter()
                .any(|artifact| !artifact.render_errors.is_empty())
            {
                // RenderFailure details name the format-level remedy (drop the
                // failing format or use its dedicated command); type it so the
                // Next line doesn't fall back to the generic output-path hint
                // (#4526/#4503-class).
                return Err(error.with_next_step(
                    "drop the failing format from --emit (or use that format's dedicated command), then retry.",
                ));
            }
            // Non-render failures (for example InstrumentFailure structural
            // gaps) cannot be fixed by dropping a format; point at the
            // manifest's reported gaps instead (#4551).
            return Err(error.with_next_step(
                "inspect the failing gaps recorded in the artifact-set manifest, resolve the reported condition, then retry.",
            ));
        }
    }
    Ok(())
}

fn reject_source_exception_options(
    compat: bool,
    kind: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<()> {
    if compat {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--compat is not supported with --profile spec-system; remove --compat or drop --profile spec-system",
        ));
    }
    if kind.is_some() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--kind is not supported with --profile spec-system; remove --kind or drop --profile spec-system",
        ));
    }
    if include_untracked {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--include-untracked is not supported with --profile spec-system; remove --include-untracked or drop --profile spec-system",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
