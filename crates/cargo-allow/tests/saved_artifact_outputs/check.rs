use super::*;

#[test]
fn saved_check_outputs_cover_report_and_receipt_contracts() {
    let fixture = SourceTreeFixture::new("saved-check-report-receipt");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let report = artifact_dir.join("check.json");
    let receipt = artifact_dir.join("check.receipt.json");

    run_cargo_allow(&[
        "check",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--mode",
        "no-new",
        "--format",
        "json",
        "--output",
        path_arg(&report),
        "--receipt",
        path_arg(&receipt),
    ]);

    let report_value =
        assert_source_syntax_artifact(&report, allow_report::REPORT_SCHEMA_ID, "check");
    let receipt_value =
        assert_source_syntax_artifact(&receipt, allow_report::RECEIPT_SCHEMA_ID, "check");
    assert_eq!(
        report_value
            .pointer("/summary/matched")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "check report matched count"
    );
    assert_eq!(
        receipt_value
            .pointer("/counts/matched")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "check receipt matched count"
    );
}
