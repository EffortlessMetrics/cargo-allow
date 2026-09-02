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

/// Configuration for the artifact set emission.
pub struct EmitConfig<'a> {
    pub operation: &'a str,
    pub formats: &'a [RendererFormatV1],
    pub result_class: EvaluationResultClassV2,
    pub blocking: bool,
    pub resolved_config_identity: &'a str,
    pub source_subject: &'a str,
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

    let set = EvaluationArtifactSetV1 {
        schema_id: "cargo-allow.evaluation-artifact-set.v1".to_string(),
        schema_version: 1,
        tool: "cargo-allow".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: operation.to_string(),
        mode: None,
        source_subject: config.source_subject.to_string(),
        resolved_config_identity: config.resolved_config_identity.to_string(),
        semantic_result_digest: semantic_result_digest(config, ctx),
        result_class: config.result_class,
        blocking: config.blocking,
        requested_formats: formats.to_vec(),
        artifacts: entries,
        core_command_summary_ref: None,
        detailed_result_ref: None,
        limitations: Vec::new(),
        claim_boundary: ("One-evaluation multi-artifact output; renderers consume the \
             same semantic result without re-evaluation."
            .to_string()),
    };

    let validation = set.validate();
    if validation.result == EvaluationArtifactSetResultV2::OutputConflict {
        return Err(format!(
            "artifact set validation detected output conflict: {:?}",
            validation.gaps
        ));
    }

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
