use super::*;

#[test]
fn saved_audit_output_covers_source_tree_report_contract() {
    let fixture = SourceTreeFixture::new("saved-audit-report");
    fixture.write_minimal_policy();
    fixture.write_panic_source();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let audit = artifact_dir.join("audit.json");

    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&audit),
    ]);

    let value = assert_source_syntax_artifact(&audit, allow_report::REPORT_SCHEMA_ID, "audit");
    assert_eq!(
        value
            .pointer("/summary/findings")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "audit findings count"
    );
    assert_eq!(
        value
            .pointer("/findings/0/path")
            .and_then(serde_json::Value::as_str),
        Some("src/lib.rs"),
        "audit finding path"
    );
}
