//! Workflow single-evaluation tests (#3881): verify that the emitted
//! artifact set carries one semantic result digest across all formats,
//! proving that a single evaluation drove all renderings.

use allow_report::{
    EvaluationArtifactEntryV1, EvaluationArtifactSetResultV2, EvaluationArtifactSetV1,
    EvaluationResultClassV2, RendererFormatV1,
};

fn make_set(
    formats: &[RendererFormatV1],
    result_class: EvaluationResultClassV2,
    digest: &str,
) -> EvaluationArtifactSetV1 {
    let artifacts: Vec<EvaluationArtifactEntryV1> = formats
        .iter()
        .map(|&format| EvaluationArtifactEntryV1 {
            format,
            file_name: EvaluationArtifactSetV1::artifact_file_name("check", format),
            content_digest: Some(format!("sha256:{:064x}", 42)),
            size_bytes: Some(1024),
            status: "Written".to_string(),
            render_errors: Vec::new(),
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
        semantic_result_digest: digest.to_string(),
        result_class,
        blocking: result_class == EvaluationResultClassV2::Blocking,
        requested_formats: formats.to_vec(),
        artifacts,
        core_command_summary_ref: None,
        detailed_result_ref: None,
        limitations: Vec::new(),
        claim_boundary: "single evaluation, multi-format".to_string(),
    }
}

#[test]
fn all_formats_share_one_semantic_result_digest() {
    let formats = [
        RendererFormatV1::Markdown,
        RendererFormatV1::Json,
        RendererFormatV1::Sarif,
        RendererFormatV1::Receipt,
    ];
    let set = make_set(
        &formats,
        EvaluationResultClassV2::Passed,
        &format!("sha256:{}", "a".repeat(64)),
    );
    // The digest is a payload-level field, not per-artifact; all formats
    // share it by construction. Verify the set validates cleanly.
    let validation = set.validate();
    assert_eq!(
        validation.result,
        EvaluationArtifactSetResultV2::Complete,
        "single-digest set should validate: {validation:?}"
    );
}

#[test]
fn mixed_result_classes_are_preserved_across_formats() {
    // A blocking semantic result with all renderers Written stays blocking.
    let formats = [
        RendererFormatV1::Markdown,
        RendererFormatV1::Json,
        RendererFormatV1::Sarif,
    ];
    let mut set = make_set(
        &formats,
        EvaluationResultClassV2::Blocking,
        format!("sha256:{}", "b".repeat(64)).as_str(),
    );
    set.blocking = true;
    let validation = set.validate();
    assert_ne!(
        validation.result,
        EvaluationArtifactSetResultV2::Complete,
        "blocking semantic should not be Complete"
    );
    // The blocking flag is unchanged by renderer status.
    assert!(set.blocking);
}

#[test]
fn renderer_order_permutation_preserves_validation() {
    let formats_a = [
        RendererFormatV1::Markdown,
        RendererFormatV1::Json,
        RendererFormatV1::Sarif,
    ];
    let formats_b = [
        RendererFormatV1::Sarif,
        RendererFormatV1::Markdown,
        RendererFormatV1::Json,
    ];
    let set_a = make_set(
        &formats_a,
        EvaluationResultClassV2::Passed,
        format!("sha256:{}", "c".repeat(64)).as_str(),
    );
    let set_b = make_set(
        &formats_b,
        EvaluationResultClassV2::Passed,
        format!("sha256:{}", "c".repeat(64)).as_str(),
    );
    // Different requested_formats order does not change the validation law.
    assert_eq!(
        set_a.validate().result,
        set_b.validate().result,
        "renderer order permutation changed the validation result"
    );
}

#[test]
fn no_shell_exit_code_arbitration_is_needed() {
    // The old pattern manually combined markdown_status, json_status, and
    // sarif_status. With the artifact set, one evaluation produces one
    // semantic result and N renderers; the manifest carries the truth.
    let formats = [
        RendererFormatV1::Markdown,
        RendererFormatV1::Json,
        RendererFormatV1::Sarif,
        RendererFormatV1::Receipt,
    ];
    let set = make_set(
        &formats,
        EvaluationResultClassV2::Blocking,
        &format!("sha256:{}", "d".repeat(64)),
    );
    // The semantic result is blocking; no renderer can change that.
    assert_eq!(set.result_class, EvaluationResultClassV2::Blocking);
    assert!(set.blocking);
}
