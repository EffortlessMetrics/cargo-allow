//! Shared multi-format artifact emitter (#3880).
//!
//! Renders one already-evaluated semantic result into N formats, writes them
//! into `--artifact-dir`, and produces a typed `EvaluationArtifactSetV1`
//! manifest. The caller evaluates exactly once; this module never re-evaluates
//! findings, policy, or exit posture.

use allow_report::{
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationResultClassV2, RendererFormatV1, render_evaluation_artifact_set_v1,
    render_html_with_context, render_json_with_context, render_markdown_with_context,
    render_receipt_with_context_and_inventory, render_sarif_with_context,
};

/// The already-evaluated semantic result that all renderers consume.
pub struct ArtifactEmitContext<'a> {
    pub command: &'a str,
    pub findings: &'a [allow_core::Finding],
    pub outcomes: &'a [allow_core::MatchOutcome],
    pub failed: bool,
    pub report_context: &'a allow_report::ReportContext<'a>,
    pub receipt_context: Option<&'a allow_report::ReportContext<'a>>,
}

/// Parse a `--emit` comma-separated value into a set of formats.
pub fn parse_emit_formats(raw: &str) -> Result<Vec<RendererFormatV1>, String> {
    let mut formats = Vec::new();
    for token in raw.split(',') {
        let token = token.trim();
        let format = match token {
            "markdown" | "md" => RendererFormatV1::Markdown,
            "json" => RendererFormatV1::Json,
            "sarif" => RendererFormatV1::Sarif,
            "html" => RendererFormatV1::Html,
            "receipt" => RendererFormatV1::Receipt,
            "human" => RendererFormatV1::HumanSummary,
            _ => return Err(format!("unsupported --emit format: {token:?}")),
        };
        formats.push(format);
    }
    Ok(formats)
}

fn render_one(format: RendererFormatV1, ctx: &ArtifactEmitContext<'_>) -> Result<Vec<u8>, String> {
    let command = ctx.command;
    let findings = ctx.findings;
    let outcomes = ctx.outcomes;
    let failed = ctx.failed;
    let rc = *ctx.report_context;
    match format {
        RendererFormatV1::Markdown => {
            Ok(render_markdown_with_context(command, findings, outcomes, failed, rc).into_bytes())
        }
        RendererFormatV1::Json => {
            Ok(render_json_with_context(command, findings, outcomes, failed, rc).into_bytes())
        }
        RendererFormatV1::Sarif => {
            Ok(render_sarif_with_context(command, findings, outcomes, failed, rc).into_bytes())
        }
        RendererFormatV1::Html => {
            Ok(render_html_with_context(command, findings, outcomes, failed, rc).into_bytes())
        }
        RendererFormatV1::Receipt => {
            // Receipt artifacts are contract-bound to the registered receipt
            // commands; another command (for example audit) must fail the
            // render honestly instead of tripping the renderer's assert
            // (#4546).
            if !allow_report::RECEIPT_COMMANDS.contains(&command) {
                return Err(format!(
                    "format receipt is not a report renderer for command {command}; \
                     receipt artifacts support only {}",
                    allow_report::RECEIPT_COMMANDS.join(", ")
                ));
            }
            let receipt_rc = ctx.receipt_context.unwrap_or(&rc);
            let text = render_receipt_with_context_and_inventory(
                command,
                findings,
                outcomes,
                failed,
                *receipt_rc,
            );
            Ok(text.into_bytes())
        }
        _ => Err(format!(
            "format {} is not a report renderer; use the dedicated command",
            format.as_str()
        )),
    }
}

fn sha256_bytes(data: &[u8]) -> String {
    allow_core::sha256_v1_bytes(data)
}

fn report_context_payload(context: &allow_report::ReportContext<'_>) -> serde_json::Value {
    serde_json::json!({
        "inventory": format!("{:?}", context.inventory),
        "baseline_debt_entries": context.baseline_debt_entries,
        "policy_missing_evidence_entries": context.policy_missing_evidence_entries,
        "broken_evidence_links": context.broken_evidence_links,
        "weak_evidence_references": context.weak_evidence_references,
        "occurrence_headroom_entries": context.occurrence_headroom_entries,
        "mode": context.mode,
        "enforcement": context.enforcement,
        "policy_config": context.policy_config,
        "tool_version": context.tool_version,
        "lane_posture": format!("{:?}", context.lane_posture),
        "federation": format!("{:?}", context.federation),
        "mirror_divergence_entries": context.mirror_divergence_entries,
        "blocking_divergence_entries": context.blocking_divergence_entries,
        "git_sha": context.git_sha,
        "policy_digest": context.policy_digest,
        "diff_analysis": format!("{:?}", context.diff_analysis),
        "rust_files_skipped": context.rust_files_skipped,
        "rust_files_considered": context.rust_files_considered,
        "rust_files_with_parse_errors": context.rust_files_with_parse_errors,
    })
}

fn semantic_result_digest(config: &EmitConfig<'_>, ctx: &ArtifactEmitContext<'_>) -> String {
    let finding_payloads = ctx
        .findings
        .iter()
        .map(|finding| {
            serde_json::json!({
                "identity": allow_core::finding_identity_key(finding),
                "message": finding.message.as_str(),
                "span": format!("{:?}", finding.span),
                "ledger": format!("{:?}", finding.ledger),
            })
        })
        .collect::<Vec<_>>();
    let outcome_identities = ctx
        .outcomes
        .iter()
        .map(|outcome| {
            format!(
                "status={:?};allow_id={:?};candidates={:?};finding_index={:?};message={};score={}",
                outcome.status,
                outcome.allow_id,
                outcome.candidate_ids,
                outcome.finding_index,
                outcome.message,
                outcome.score
            )
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "operation": config.operation,
        "command": ctx.command,
        "source_subject": config.source_subject,
        "resolved_config_identity": config.resolved_config_identity,
        "result_class": format!("{:?}", config.result_class),
        "blocking": config.blocking,
        "failed": ctx.failed,
        "report_context": report_context_payload(ctx.report_context),
        "receipt_context": ctx.receipt_context.map(report_context_payload),
        "finding_payloads": finding_payloads,
        "outcome_identities": outcome_identities,
    });
    sha256_bytes(&serde_json::to_vec(&payload).unwrap_or_default())
}

/// Cite the summary sidecar root-relative when it lives under the source
/// root, so the manifest ref is stable across checkout locations (#4597);
/// otherwise cite the caller's path as given. Separators normalize to `/`.
fn summary_ref_text(path: &std::path::Path, ctx: &ArtifactEmitContext<'_>) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(root) = ctx.report_context.inventory.root {
        if let Ok(relative) = path.strip_prefix(root) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }
    text
}

/// Configuration for the artifact set emission.
pub struct EmitConfig<'a> {
    pub operation: &'a str,
    pub formats: &'a [RendererFormatV1],
    pub result_class: EvaluationResultClassV2,
    pub blocking: bool,
    pub resolved_config_identity: &'a str,
    pub source_subject: &'a str,
    /// The invoking check's mode (`no-new`, ...), when the command has one
    /// (#4597). Audit and diff have no mode and leave `None`.
    pub mode: Option<&'a str>,
}

/// The common source-tree inventory facts of one evaluation, bound from
/// the report context so the artifact-set manifest carries the same
/// governed inventory shape as receipts and reports (#4560 alignment).
fn inventory_value_for_artifacts(
    ctx: &ArtifactEmitContext<'_>,
) -> allow_report::EvaluationInventoryV1 {
    let inventory = &ctx.report_context.inventory;
    allow_report::EvaluationInventoryV1 {
        scope: inventory.scope.to_string(),
        scanner: inventory.scanner.to_string(),
        source: inventory.source.to_string(),
        root: inventory.root.map(str::to_string),
        files_scanned: inventory.files_scanned.map(|count| count as u64),
        empty_git_tracked: inventory.empty_git_tracked,
        completeness: inventory.completeness.map(str::to_string),
        source_identity: inventory.source_identity.map(str::to_string),
    }
}

/// Refuse an artifact directory that would contain the selected policy
/// file: emitted report files beside the live ledger is a collision the
/// OutputConflict classification could never see (its detection was
/// unreachable dead code, removed with #4561). Ordering scope: this runs
/// before the artifact writes only — each command's report and receipt
/// outputs are governed by their own containment assertions earlier in
/// the command.
///
/// Resolution is OS-accurate for the directory: symlinks in existing
/// prefixes resolve through the OS (canonicalize) and only segments after
/// the deepest existing ancestor are folded lexically. Comparison is
/// component-wise on the resolved paths, so non-UTF-8 spellings never
/// collapse (#4576 review).
pub(crate) fn ensure_artifact_dir_separate(
    artifact_dir: &std::path::Path,
    protected_file: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some(protected) = protected_file else {
        return Ok(());
    };
    let resolved_dir = best_effort_canonical(artifact_dir);
    let resolved_protected = best_effort_canonical(protected);
    if resolved_protected.starts_with(&resolved_dir) {
        return Err(format!(
            "the artifact directory would contain the selected policy file ({} inside {}); choose a directory outside the policy location",
            protected.display(),
            artifact_dir.display()
        ));
    }
    Ok(())
}

/// Resolve `path` as far as the OS can: canonicalize the deepest existing
/// ancestor (which follows symlinks and resolves `..` natively), then
/// append the not-yet-existing tail with `.`/`..` folded lexically. Never
/// pre-folds `..` across an existing symlinked segment — the OS would
/// resolve that through the link target (#4576 review).
fn best_effort_canonical(path: &std::path::Path) -> std::path::PathBuf {
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path.to_path_buf();
    loop {
        if let Ok(resolved) = cursor.canonicalize() {
            let mut result = resolved;
            for component in missing.iter().rev() {
                if component.as_os_str() == ".." {
                    result.pop();
                } else if component.as_os_str() != "." {
                    result.push(component);
                }
            }
            return result;
        }
        match cursor.file_name() {
            Some(name) => {
                missing.push(name.to_os_string());
                if !cursor.pop() {
                    return path.to_path_buf();
                }
            }
            // file_name() is None both at the filesystem root and for a
            // path terminating in `..`. Only the `..` ending folds: the OS
            // would create the missing segment and pop it, netting the
            // parent (#4583 review). A root cursor with nothing left just
            // returns the walk input.
            None => {
                let fold = cursor.ends_with("..");
                if !cursor.pop() && !fold {
                    return path.to_path_buf();
                }
                if fold {
                    missing.push(std::ffi::OsString::from(".."));
                }
                continue;
            }
        }
    }
}

/// Render and write the requested artifacts into `artifact_dir`, producing a
/// typed `EvaluationArtifactSetV1` manifest.
pub fn emit_artifact_set(
    artifact_dir: &std::path::Path,
    config: &EmitConfig<'_>,
    ctx: &ArtifactEmitContext<'_>,
) -> Result<EvaluationArtifactSetV1, String> {
    let operation = config.operation;
    let formats = config.formats;
    std::fs::create_dir_all(artifact_dir)
        .map_err(|error| format!("create artifact dir: {error}"))?;

    let mut entries = Vec::new();
    for &format in formats {
        let file_name = EvaluationArtifactSetV1::artifact_file_name(operation, format);
        let path = artifact_dir.join(&file_name);

        let rendered = render_one(format, ctx);
        match rendered {
            Ok(bytes) => {
                std::fs::write(&path, &bytes)
                    .map_err(|error| format!("write {}: {error}", path.display()))?;
                entries.push(EvaluationArtifactEntryV1 {
                    format,
                    file_name: file_name.clone(),
                    content_digest: Some(sha256_bytes(&bytes)),
                    size_bytes: Some(bytes.len() as u64),
                    status: "Written".to_string(),
                    render_errors: Vec::new(),
                });
            }
            Err(error) => {
                entries.push(EvaluationArtifactEntryV1 {
                    format,
                    file_name: file_name.clone(),
                    content_digest: None,
                    size_bytes: None,
                    status: "RenderFailed".to_string(),
                    render_errors: vec![error],
                });
            }
        }
    }

    // The command-specific detailed result is the JSON detail artifact the
    // same emission just wrote (#4597); a run that did not request json
    // (or whose json render failed) has no detailed result to cite.
    let detailed_result_ref = entries
        .iter()
        .find(|entry| entry.format == RendererFormatV1::Json && entry.status == "Written")
        .map(|entry| entry.file_name.clone());
    let mut set = EvaluationArtifactSetV1 {
        schema_id: "cargo-allow.evaluation-artifact-set.v1".to_string(),
        schema_version: 1,
        tool: "cargo-allow".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: operation.to_string(),
        command: ctx.command.to_string(),
        mode: config.mode.map(str::to_string),
        inventory: inventory_value_for_artifacts(ctx),
        scanner_limitations: allow_report::SCANNER_LIMITATIONS
            .iter()
            .map(|limitation| limitation.to_string())
            .collect(),
        source_subject: config.source_subject.to_string(),
        resolved_config_identity: config.resolved_config_identity.to_string(),
        semantic_result_digest: semantic_result_digest(config, ctx),
        result_class: config.result_class,
        blocking: config.blocking,
        requested_formats: formats.to_vec(),
        artifacts: entries,
        // The invocation's own --command-summary-output sidecar, when the
        // flag configured one (#4393); cited so the manifest links the
        // summary consumers were told to read (#4597). Cited root-relative
        // when it lives under the source root, so the ref is stable across
        // checkout locations.
        core_command_summary_ref: crate::core_command_router::requested_summary_path()
            .map(|path| summary_ref_text(path, ctx)),
        detailed_result_ref,
        limitations: Vec::new(),
        claim_boundary: allow_report::claim_boundary_for_schema_id(
            allow_report::EVALUATION_ARTIFACT_SET_V1_SCHEMA_ID,
        )
        .iter()
        .map(|flag| flag.to_string())
        .collect(),
        validation_gaps: Vec::new(),
        // Placeholder until validate() runs below; the manifest is rendered
        // only after the computed verdict is written back (#4595).
        validation_result: EvaluationArtifactSetResultV2::Complete,
    };

    let validation = set.validate();
    // The OutputConflict arm is unreachable here: limitations are never
    // populated by this emitter, and the real collision (artifacts beside
    // the selected policy) is refused by ensure_artifact_dir_separate
    // before any write (#4561).
    debug_assert_ne!(
        validation.result,
        EvaluationArtifactSetResultV2::OutputConflict
    );
    // #4560: a non-Complete set's persisted manifest must carry the failing
    // gaps, so the typed Next line ("inspect the failing gaps recorded in
    // the artifact-set manifest") points at real recorded content.
    if !validation.gaps.is_empty() {
        set.validation_gaps = validation.gaps.clone();
    }
    // #4595: serialize the computed verdict so a schema-only gate can tell a
    // refused run from a green one without recomputing validate().
    set.validation_result = validation.result;

    let manifest_name = EvaluationArtifactSetV1::artifact_file_name(
        operation,
        RendererFormatV1::ArtifactSetManifest,
    );
    let manifest = render_evaluation_artifact_set_v1(&set)
        .map_err(|error| format!("render manifest: {error}"))?;
    std::fs::write(artifact_dir.join(&manifest_name), manifest)
        .map_err(|error| format!("write manifest: {error}"))?;

    Ok(set)
}

#[cfg(test)]
#[path = "artifact_emit_tests.rs"]
mod tests;
