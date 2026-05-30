use super::*;

#[test]
fn saved_propose_output_allows_broken_evidence_baseline() {
    let fixture = SourceTreeFixture::new("saved-propose-broken-evidence");
    fixture.write_policy_with_broken_evidence();
    fixture.write_panic_source();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let proposed_policy = artifact_dir.join("allow.proposed.toml");
    let propose_summary = artifact_dir.join("propose-summary.json");

    run_cargo_allow(&[
        "propose",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--write",
        path_arg(&proposed_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&propose_summary),
    ]);
    assert_policy_output(&proposed_policy);
    let value =
        assert_source_syntax_artifact(&propose_summary, allow_report::PROPOSE_SCHEMA_ID, "propose");
    assert_eq!(
        value
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "propose should still generate the new panic baseline entry"
    );
}
