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

#[test]
fn saved_check_outputs_fail_and_route_redundant_segment_evidence_scope() {
    let fixture = SourceTreeFixture::new("saved-check-redundant-evidence-scope");
    fixture.write_policy_with_redundant_segment_evidence_scope();
    fixture.write_unsafe_source();

    let (report_value, receipt_value) = run_redundant_check(&fixture, "redundant-evidence-scope");
    assert_redundant_check(
        &report_value,
        &receipt_value,
        "redundant local evidence path segments",
    );
}

#[test]
fn saved_check_outputs_fail_and_route_redundant_segment_link_scope() {
    let fixture = SourceTreeFixture::new("saved-check-redundant-link-scope");
    fixture.write_policy_with_redundant_segment_link_scope();
    fixture.write_unsafe_source();

    let (report_value, receipt_value) = run_redundant_check(&fixture, "redundant-link-scope");
    assert_redundant_check(
        &report_value,
        &receipt_value,
        "redundant local link path segments",
    );
}

fn run_redundant_check(
    fixture: &SourceTreeFixture,
    name: &str,
) -> (serde_json::Value, serde_json::Value) {
    let artifact_dir = fixture.root.join("target/cargo-allow");
    let report = artifact_dir.join(format!("check-{name}.json"));
    let receipt = artifact_dir.join(format!("check-{name}.receipt.json"));

    run_cargo_allow_expect_status(
        &[
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
        ],
        false,
    );

    (
        assert_source_syntax_artifact(&report, allow_report::REPORT_SCHEMA_ID, "check"),
        assert_source_syntax_artifact(&receipt, allow_report::RECEIPT_SCHEMA_ID, "check"),
    )
}

fn assert_redundant_check(
    report_value: &serde_json::Value,
    receipt_value: &serde_json::Value,
    label: &str,
) {
    assert_eq!(
        report_value
            .pointer("/failed")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "check report should fail closed on {label}"
    );
    assert_eq!(
        receipt_value
            .pointer("/failed")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "check receipt should fail closed on {label}"
    );
    assert_eq!(
        report_value
            .pointer("/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "check report should count {label} as broken evidence"
    );
    assert_eq!(
        report_value
            .pointer("/trend/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "check trend should count {label} as broken evidence"
    );
    assert_eq!(
        receipt_value
            .pointer("/counts/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "check receipt should count {label} as broken evidence"
    );
    assert_evidence_repair_queue(report_value, "/evidence_repair_queues", "check report");
    assert_evidence_repair_queue(receipt_value, "/evidence_repair_queues", "check receipt");
}

fn assert_evidence_repair_queue(value: &serde_json::Value, pointer: &str, label: &str) {
    let queues = value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{label} should route evidence repair queues")
        });
    assert!(
        queues.iter().any(|queue| {
            queue.get("signal").and_then(serde_json::Value::as_str) == Some("broken_evidence_links")
                && queue.get("route_kind").and_then(serde_json::Value::as_str)
                    == Some("worklist_filter")
                && queue.get("item_kind").and_then(serde_json::Value::as_str)
                    == Some("broken_evidence_link")
                && queue
                    .get("worklist_filter")
                    .and_then(serde_json::Value::as_str)
                    == Some("broken_evidence")
                && queue.get("count").and_then(serde_json::Value::as_u64) == Some(1)
                && queue.get("command").and_then(serde_json::Value::as_str)
                    == Some("cargo-allow worklist --broken-evidence --format json")
        }),
        "{label} should route broken evidence through the focused worklist shortcut"
    );
}
