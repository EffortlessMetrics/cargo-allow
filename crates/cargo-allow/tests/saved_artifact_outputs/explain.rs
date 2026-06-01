use super::*;

#[test]
fn saved_explain_output_allows_broken_evidence_diagnostics() {
    let fixture = SourceTreeFixture::new("saved-explain-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let explain = artifact_dir.join("explain.json");

    run_cargo_allow(&[
        "explain",
        "allow-broken-evidence",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&explain),
    ]);
    let value = assert_source_syntax_artifact(&explain, allow_report::EXPLAIN_SCHEMA_ID, "explain");
    assert_eq!(
        value
            .pointer("/allow_entry/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "explain should still load the requested broken-evidence entry"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(serde_json::Value::as_str),
        Some("local_file_missing"),
        "explain should surface the broken local evidence diagnostic"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/missing-evidence.md"),
        "explain should preserve the source-tree evidence target"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/next/proof_commands");
}

#[test]
fn saved_explain_output_reports_present_and_traceability_evidence() {
    let fixture = SourceTreeFixture::new("saved-explain-evidence-diagnostics");
    fixture.write_policy_with_present_and_traceability_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let explain = artifact_dir.join("explain-evidence.json");

    run_cargo_allow(&[
        "explain",
        "allow-evidence-diagnostics",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&explain),
    ]);
    let value = assert_source_syntax_artifact(&explain, allow_report::EXPLAIN_SCHEMA_ID, "explain");
    assert_eq!(
        value
            .pointer("/allow_entry/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-evidence-diagnostics"),
        "explain should load the requested evidence fixture"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(serde_json::Value::as_str),
        Some("local_file_present"),
        "explain should surface present local evidence"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/evidence/safety.md"),
        "explain should preserve the local evidence target"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/1/status")
            .and_then(serde_json::Value::as_str),
        Some("traceability_only"),
        "explain should keep test evidence as traceability-only"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/1/prefix")
            .and_then(serde_json::Value::as_str),
        Some("test"),
        "explain should preserve the traceability evidence prefix"
    );
    assert_eq!(
        value
            .pointer("/next/suggested_actions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "present and traceability evidence should not create repair actions"
    );
}

#[test]
fn saved_explain_output_preserves_invalid_evidence_target() {
    let fixture = SourceTreeFixture::new("saved-explain-invalid-evidence-scope");
    fixture.write_policy_with_invalid_evidence_scope();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let explain = artifact_dir.join("explain-invalid-evidence.json");

    run_cargo_allow(&[
        "explain",
        "allow-invalid-evidence-scope",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&explain),
    ]);
    let value = assert_source_syntax_artifact(&explain, allow_report::EXPLAIN_SCHEMA_ID, "explain");
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(serde_json::Value::as_str),
        Some("invalid_local_path"),
        "explain should surface invalid local evidence target status"
    );
    assert_eq!(
        value
            .pointer("/evidence_references/0/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/../src/lib.rs"),
        "explain should preserve invalid target text instead of normalizing it into a valid-looking path"
    );
}
