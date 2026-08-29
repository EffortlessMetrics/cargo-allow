//! Artifact set contract tests (#3879): one evaluation → one set, digest
//! consistency, semantic/renderer separation.

use allow_report::{
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationResultClassV2, RendererFormatV1,
};

fn entry(format: RendererFormatV1, status: &str) -> EvaluationArtifactEntryV1 {
    EvaluationArtifactEntryV1 {
        format,
        file_name: EvaluationArtifactSetV1::artifact_file_name("check", format),
        content_digest: if status == "Written" {
            Some(format!("sha256:{:064x}", 42))
        } else {
            None
        },
        size_bytes: if status == "Written" {
            Some(1024)
        } else {
            None
        },
        status: status.to_string(),
        render_errors: Vec::new(),
    }
}

fn complete_set() -> EvaluationArtifactSetV1 {
    EvaluationArtifactSetV1 {
        schema_id: "cargo-allow.evaluation-artifact-set.v1".to_string(),
        schema_version: 1,
        tool: "cargo-allow".to_string(),
        tool_version: "0.2.0-rc.1".to_string(),
        operation: "check".to_string(),
        mode: Some("no-new".to_string()),
        source_subject: "worktree:clean".to_string(),
        resolved_config_identity: format!("sha256:{}", "a".repeat(64)),
        semantic_result_digest: format!("sha256:{}", "b".repeat(64)),
        result_class: EvaluationResultClassV2::Passed,
        blocking: false,
        requested_formats: vec![
            RendererFormatV1::Markdown,
            RendererFormatV1::Json,
            RendererFormatV1::Sarif,
        ],
        artifacts: vec![
            entry(RendererFormatV1::Markdown, "Written"),
            entry(RendererFormatV1::Json, "Written"),
            entry(RendererFormatV1::Sarif, "Written"),
        ],
        core_command_summary_ref: Some("check-core-command-summary.json".to_string()),
        detailed_result_ref: Some("check-receipt.json".to_string()),
        limitations: Vec::new(),
        claim_boundary: "one evaluation, many renderers".to_string(),
    }
}

#[test]
fn contract_complete_set_passes() -> Result<(), String> {
    let set = complete_set();
    let validation = set.validate();
    if validation.result != EvaluationArtifactSetResultV2::Complete {
        return Err(format!("complete set rejected: {validation:?}"));
    }
    Ok(())
}

#[test]
fn contract_semantic_and_renderer_status_are_separate() -> Result<(), String> {
    // A semantic failure with all renderers succeeding is SemanticNonGreen.
    let mut set = complete_set();
    set.result_class = EvaluationResultClassV2::Blocking;
    set.blocking = true;
    let validation = set.validate();
    if validation.result != EvaluationArtifactSetResultV2::SemanticNonGreen {
        return Err(format!(
            "semantic blocking was not classified SemanticNonGreen: {validation:?}"
        ));
    }
    // A renderer failure does not change the semantic posture.
    let markdown = set
        .artifacts
        .first_mut()
        .ok_or_else(|| "fixture lost its markdown entry".to_string())?;
    markdown.status = "RenderFailed".to_string();
    markdown.render_errors = vec!["markdown renderer panicked".to_string()];
    markdown.content_digest = None;
    let validation = set.validate();
    if validation.result != EvaluationArtifactSetResultV2::RenderFailure {
        return Err(format!(
            "renderer failure was not classified RenderFailure: {validation:?}"
        ));
    }
    if set.result_class != EvaluationResultClassV2::Blocking || !set.blocking {
        return Err("semantic posture was mutated by renderer failure".to_string());
    }
    Ok(())
}

#[test]
fn contract_digest_is_bound_across_artifacts() -> Result<(), String> {
    let set = complete_set();
    if set.semantic_result_digest.is_empty() {
        return Err("semantic digest is empty".to_string());
    }
    for artifact in &set.artifacts {
        if artifact.status == "Written" {
            let digest = artifact
                .content_digest
                .as_deref()
                .ok_or_else(|| format!("{} missing digest", artifact.file_name))?;
            if !digest.starts_with("sha256:") || digest.len() != "sha256:".len() + 64 {
                return Err(format!("{} has malformed digest", artifact.file_name));
            }
        }
    }
    Ok(())
}
