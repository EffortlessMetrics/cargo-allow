use super::*;

#[test]
fn saved_list_output_allows_broken_evidence_entries() {
    let fixture = SourceTreeFixture::new("saved-list-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let list = artifact_dir.join("list.json");

    run_cargo_allow(&[
        "list",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&list),
    ]);
    let value = assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "list should still browse the broken-evidence ledger entry"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "list should include the retained allow entry"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/evidence_count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "list should preserve the evidence reference count"
    );
}

#[test]
fn saved_list_output_filters_policy_missing_evidence_entries() {
    let fixture = SourceTreeFixture::new("saved-list-missing-evidence");
    fixture.write_policy_with_missing_evidence_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let list = artifact_dir.join("list-missing-evidence.json");

    run_cargo_allow(&[
        "list",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--missing-evidence",
        "--format",
        "json",
        "--output",
        path_arg(&list),
    ]);
    let value = assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "list should contain one missing-evidence policy entry"
    );
    assert_eq!(
        value
            .pointer("/filters/missing_evidence")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "list artifact should preserve the missing-evidence filter"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-missing-evidence"),
        "list allow id"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/status")
            .and_then(serde_json::Value::as_str),
        Some("matched"),
        "list row status should remain matched"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/evidence_count")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "list evidence count"
    );
}

#[test]
fn saved_list_output_filters_policy_entries_by_allow_id() {
    let fixture = SourceTreeFixture::new("saved-list-allow-id");
    fixture.write_policy_with_missing_evidence_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let list = artifact_dir.join("list-allow-id.json");

    run_cargo_allow(&[
        "list",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--allow-id",
        "allow-missing-evidence",
        "--format",
        "json",
        "--output",
        path_arg(&list),
    ]);
    let value = assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "list should contain one allow-id-selected policy entry"
    );
    assert_eq!(
        value
            .pointer("/filters/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-missing-evidence"),
        "list artifact should preserve the allow-id filter"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-missing-evidence"),
        "list allow id"
    );
}

#[test]
fn saved_list_output_filters_policy_baseline_debt_entries() {
    let fixture = SourceTreeFixture::new("saved-list-baseline-debt");
    fixture.write_policy_with_baseline_debt_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let list = artifact_dir.join("list-baseline-debt.json");

    run_cargo_allow(&[
        "list",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--baseline-debt",
        "--format",
        "json",
        "--output",
        path_arg(&list),
    ]);
    let value = assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "list should contain one baseline-debt policy entry"
    );
    assert_eq!(
        value
            .pointer("/filters/baseline_debt")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "list artifact should preserve the baseline-debt filter"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-baseline-debt"),
        "list allow id"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/status")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "list row status"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/classification")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "list classification"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/evidence_count")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "list evidence count"
    );
}
