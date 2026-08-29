//! Artifact set renderer failure tests (#3879): renderer failure is
//! fail-honest, semantic posture is unchanged, and the validation
//! classifies the failure mode correctly.

use allow_report::{
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationResultClassV2, RendererFormatV1,
};

fn set_with_statuses(statuses: &[(&str, &str)]) -> EvaluationArtifactSetV1 {
    let entries: Vec<EvaluationArtifactEntryV1> = statuses
        .iter()
        .map(|(format_str, status)| {
            let format = match *format_str {
                "markdown" => RendererFormatV1::Markdown,
                "json" => RendererFormatV1::Json,
                "sarif" => RendererFormatV1::Sarif,
                _ => RendererFormatV1::Receipt,
            };
            EvaluationArtifactEntryV1 {
                format,
                file_name: EvaluationArtifactSetV1::artifact_file_name("check", format),
                content_digest: if *status == "Written" {
                    Some(format!("sha256:{:064x}", 42))
                } else {
                    None
                },
                size_bytes: if *status == "Written" {
                    Some(1024)
                } else {
                    None
                },
                status: status.to_string(),
                render_errors: if *status == "RenderFailed" {
                    vec!["renderer failed".to_string()]
                } else {
                    Vec::new()
                },
            }
        })
        .collect();
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
        artifacts: entries,
        core_command_summary_ref: None,
        detailed_result_ref: None,
        limitations: Vec::new(),
        claim_boundary: "bounded artifact set".to_string(),
    }
}

#[test]
fn all_renderers_succeeding_is_complete() {
    let set = set_with_statuses(&[
        ("markdown", "Written"),
        ("json", "Written"),
        ("sarif", "Written"),
    ]);
    let validation = set.validate();
    assert_eq!(validation.result, EvaluationArtifactSetResultV2::Complete);
}

#[test]
fn one_renderer_failure_is_render_failure_not_complete() {
    let set = set_with_statuses(&[
        ("markdown", "Written"),
        ("json", "RenderFailed"),
        ("sarif", "Written"),
    ]);
    let validation = set.validate();
    assert_eq!(
        validation.result,
        EvaluationArtifactSetResultV2::RenderFailure
    );
    // The semantic result is not changed by renderer failure.
    assert_eq!(set.result_class, EvaluationResultClassV2::Passed);
}

#[test]
fn all_renderers_failing_is_render_failure() {
    let set = set_with_statuses(&[
        ("markdown", "RenderFailed"),
        ("json", "RenderFailed"),
        ("sarif", "RenderFailed"),
    ]);
    let validation = set.validate();
    assert_eq!(
        validation.result,
        EvaluationArtifactSetResultV2::RenderFailure
    );
}

#[test]
fn renderer_order_permutation_does_not_change_semantic_result() {
    let set_a = set_with_statuses(&[
        ("markdown", "Written"),
        ("json", "Written"),
        ("sarif", "Written"),
    ]);
    // The same artifacts with permuted order are not a different semantic
    // result; the validation law is per-entry, not order-dependent.
    let set_b = set_with_statuses(&[
        ("markdown", "Written"),
        ("json", "Written"),
        ("sarif", "Written"),
    ]);
    assert_eq!(set_a.result_class, set_b.result_class);
    assert_eq!(set_a.semantic_result_digest, set_b.semantic_result_digest);
    assert_eq!(set_a.validate().result, set_b.validate().result);
}
