use super::*;

#[test]
fn saved_add_output_records_selected_finding_and_policy_path() {
    let fixture = SourceTreeFixture::new("saved-add-summary");
    fixture.write_minimal_policy();
    fixture.write_panic_source();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let proposed_policy = artifact_dir.join("allow.added.toml");
    let add_summary = artifact_dir.join("add-summary.json");

    run_cargo_allow(&[
        "add",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "1",
        "--owner",
        "core/tests",
        "--reason",
        "Fixture keeps add saved artifact output covered.",
        "--evidence",
        "test:saved_add_output_records_selected_finding_and_policy_path",
        "--write",
        path_arg(&proposed_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&add_summary),
    ]);

    assert_policy_output(&proposed_policy);
    let value = assert_source_syntax_artifact(&add_summary, allow_report::ADD_SCHEMA_ID, "add");
    assert_eq!(
        value
            .pointer("/summary/human_review_required")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "add summary should require human review"
    );
    assert_eq!(
        value
            .pointer("/allow_entry/kind")
            .and_then(serde_json::Value::as_str),
        Some("panic"),
        "add allow entry kind"
    );
    assert_eq!(
        value
            .pointer("/selected_finding/path")
            .and_then(serde_json::Value::as_str),
        Some("src/lib.rs"),
        "add selected finding path"
    );
}
