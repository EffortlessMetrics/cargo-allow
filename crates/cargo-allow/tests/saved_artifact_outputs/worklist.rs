use super::*;

#[test]
fn saved_worklist_output_includes_broken_evidence_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let value = run_broken_evidence_worklist(&fixture);
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "worklist should contain one broken evidence item"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link"),
        "worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "worklist allow id"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/raw")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/missing-evidence.md"),
        "worklist evidence reference raw value"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("local_file_missing"),
        "worklist evidence reference status"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/1")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow list --allow-id allow-broken-evidence --format json"),
        "worklist list allow-id proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/2")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --allow-id allow-broken-evidence --format json"),
        "worklist worklist allow-id proof command"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_includes_policy_missing_evidence_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-missing-evidence");
    fixture.write_policy_with_missing_evidence_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-missing-evidence.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--missing-evidence",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    let value =
        assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "worklist should contain one policy missing-evidence item"
    );
    assert_eq!(
        value
            .pointer("/filters/missing_evidence")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "worklist artifact should preserve the missing-evidence filter"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("missing_evidence"),
        "worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/status")
            .and_then(serde_json::Value::as_str),
        Some("evidence_missing"),
        "worklist item status"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-missing-evidence"),
        "worklist allow id"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_count")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "worklist evidence count"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/1")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow list --allow-id allow-missing-evidence --format json"),
        "worklist list allow-id proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/2")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --allow-id allow-missing-evidence --format json"),
        "worklist worklist allow-id proof command"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_includes_weak_evidence_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-weak-evidence");
    fixture.write_policy_with_weak_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-weak-evidence.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--item-kind",
        "weak_evidence_reference",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    let value =
        assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "worklist should contain one weak evidence item"
    );
    assert_eq!(
        value
            .pointer("/filters/item_kind")
            .and_then(serde_json::Value::as_str),
        Some("weak_evidence_reference"),
        "worklist artifact should preserve the weak-evidence item-kind filter"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("weak_evidence_reference"),
        "worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-weak-evidence"),
        "worklist allow id"
    );
    assert!(
        value
            .pointer("/work_items/0/path")
            .is_some_and(serde_json::Value::is_null),
        "weak evidence work items should not expose non-source-tree evidence targets as paths"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/raw")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet:manual-review"),
        "worklist evidence reference raw value"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/prefix")
            .and_then(serde_json::Value::as_str),
        Some("spreadsheet"),
        "worklist evidence reference prefix"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/target")
            .and_then(serde_json::Value::as_str),
        Some("manual-review"),
        "worklist evidence reference target"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("unstructured"),
        "worklist evidence reference status"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_includes_policy_baseline_debt_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-baseline-debt");
    fixture.write_policy_with_baseline_debt_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-baseline-debt.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--baseline-debt",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    let value =
        assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "worklist should contain one baseline-debt policy item"
    );
    assert_eq!(
        value
            .pointer("/filters/baseline_debt")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "worklist artifact should preserve the baseline-debt filter"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/status")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "worklist item status"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-baseline-debt"),
        "worklist allow id"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/classification")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "worklist classification"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/0")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow explain allow-baseline-debt"),
        "worklist explain proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/1")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow list --allow-id allow-baseline-debt --format json"),
        "worklist list allow-id proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/2")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --allow-id allow-baseline-debt --format json"),
        "worklist worklist allow-id proof command"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_includes_invalid_evidence_scope_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-invalid-evidence-scope");
    fixture.write_policy_with_invalid_evidence_scope();

    let value = run_broken_evidence_worklist(&fixture);
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "worklist should contain one invalid evidence scope item"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link"),
        "worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/path")
            .and_then(serde_json::Value::as_str),
        Some("docs/../src/lib.rs"),
        "worklist should expose the invalid source-tree-relative evidence target"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/../src/lib.rs"),
        "worklist evidence diagnostic should not normalize away invalid parent segments"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("invalid_local_path"),
        "worklist should carry the structured invalid evidence diagnostic"
    );
    let message = value
        .pointer("/work_items/0/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("worklist item should include message"));
    assert!(
        message.contains("parent directory segments"),
        "worklist should explain why the local evidence scope is invalid: {message}"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

fn run_broken_evidence_worklist(fixture: &SourceTreeFixture) -> serde_json::Value {
    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--item-kind",
        "broken_evidence_link",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist")
}
