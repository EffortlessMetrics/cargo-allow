use super::*;

#[test]
fn saved_diff_output_covers_clean_posture_report_contract() {
    let fixture = SourceTreeFixture::new("saved-diff-clean");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();
    commit_fixture_base(&fixture.root);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow(&[
        "diff",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--base",
        "HEAD",
        "--format",
        "json",
        "--output",
        path_arg(&diff),
    ]);

    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );
    assert_eq!(
        value
            .pointer("/diff/summary/new_findings")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "diff new findings"
    );
}
