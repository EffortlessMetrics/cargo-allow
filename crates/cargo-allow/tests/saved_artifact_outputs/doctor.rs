use super::*;

#[test]
fn saved_doctor_output_reports_broken_evidence_config_diagnostic() {
    let fixture = SourceTreeFixture::new("saved-doctor-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let doctor = artifact_dir.join("doctor.json");

    run_cargo_allow(&[
        "doctor",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&doctor),
    ]);
    let value = assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value
            .pointer("/config/valid")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "doctor should mark config invalid when local evidence is broken"
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(
        diagnostic.contains("allow-broken-evidence evidence"),
        "doctor diagnostic should identify the allow entry: {diagnostic}"
    );
    assert!(
        diagnostic.contains("docs/missing-evidence.md"),
        "doctor diagnostic should include the missing evidence path: {diagnostic}"
    );
}
