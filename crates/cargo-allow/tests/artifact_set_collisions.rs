//! Artifact set collision tests (#3879): output collision, partial write,
//! and policy/source collision detection.

use allow_report::{
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationResultClassV2, RendererFormatV1,
};

#[test]
fn collision_detection_catches_duplicate_paths() {
    let collisions = EvaluationArtifactSetV1::detect_collisions(
        &["out/check.md".to_string(), "out/check.md".to_string()],
        &[],
    );
    assert_eq!(collisions.len(), 1);
    assert!(collisions[0].contains("duplicate"));
}

#[test]
fn collision_detection_catches_policy_source_overlap() {
    let collisions = EvaluationArtifactSetV1::detect_collisions(
        &["policy/allow.toml".to_string()],
        &["policy/allow.toml".to_string()],
    );
    assert!(!collisions.is_empty());
    assert!(collisions[0].contains("reserved"));
}

#[test]
fn collision_detection_passes_for_disjoint_paths() {
    let collisions = EvaluationArtifactSetV1::detect_collisions(
        &[
            "out/check.md".to_string(),
            "out/check.json".to_string(),
            "out/check.sarif".to_string(),
        ],
        &["policy/allow.toml".to_string()],
    );
    assert!(collisions.is_empty());
}

#[test]
fn partial_artifact_set_is_fail_honest() {
    // One of three renderers fails → PartialArtifacts, not Complete.
    let entries = vec![
        entry_status(RendererFormatV1::Markdown, "Written"),
        entry_status(RendererFormatV1::Json, "Written"),
        entry_status(RendererFormatV1::Sarif, "RenderFailed"),
    ];
    let set = EvaluationArtifactSetV1 {
        schema_id: "cargo-allow.evaluation-artifact-set.v1".to_string(),
        schema_version: 1,
        tool: "cargo-allow".to_string(),
        tool_version: "0.2.0-rc.1".to_string(),
        operation: "check".to_string(),
        mode: None,
        source_subject: "worktree".to_string(),
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
        limitations: vec!["SARIF renderer failed".to_string()],
        claim_boundary: "bounded artifact set".to_string(),
    };
    let validation = set.validate();
    assert_eq!(
        validation.result,
        EvaluationArtifactSetResultV2::RenderFailure,
        "partial write should be RenderFailure: {validation:?}"
    );
}

fn entry_status(format: RendererFormatV1, status: &str) -> EvaluationArtifactEntryV1 {
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
        render_errors: if status == "RenderFailed" {
            vec!["renderer failed".to_string()]
        } else {
            Vec::new()
        },
    }
}
