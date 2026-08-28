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
fn saved_worklist_default_output_includes_policy_missing_evidence_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-default-missing-evidence");
    fixture.write_policy_with_missing_evidence_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
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
        "default worklist should contain one policy missing-evidence item"
    );
    assert_eq!(
        value
            .pointer("/filters/missing_evidence")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "default worklist artifact should preserve the inactive missing-evidence filter"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("missing_evidence"),
        "default worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/summary/item_kinds/missing_evidence")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "default worklist summary should count missing-evidence queue items"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-missing-evidence"),
        "default worklist allow id"
    );
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
fn saved_worklist_output_routes_unsafe_baseline_debt_closeout() {
    let fixture = SourceTreeFixture::new("saved-worklist-unsafe-baseline-debt");
    fixture.write_policy_with_unsafe_baseline_debt_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-unsafe-baseline-debt.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "unsafe",
        "--item-kind",
        "baseline_debt",
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
        "unsafe baseline-debt closeout should contain one item"
    );
    assert_eq!(
        value
            .pointer("/filters/kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "worklist artifact should preserve the unsafe kind filter"
    );
    assert_eq!(
        value
            .pointer("/filters/item_kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "worklist artifact should preserve the baseline-debt item-kind filter"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "unsafe baseline-debt worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/exception_kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "unsafe baseline-debt exception kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/family")
            .and_then(serde_json::Value::as_str),
        Some("unsafe_block"),
        "unsafe baseline-debt family"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/risk")
            .and_then(serde_json::Value::as_str),
        Some("high"),
        "unsafe baseline-debt risk"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/difficulty")
            .and_then(serde_json::Value::as_str),
        Some("medium"),
        "unsafe baseline-debt difficulty"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/status")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "unsafe baseline-debt status"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-unsafe-baseline-debt"),
        "unsafe baseline-debt allow id"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/classification")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "unsafe baseline-debt classification"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "unsafe baseline-debt weak placeholder evidence count"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/0")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow explain allow-unsafe-baseline-debt"),
        "unsafe baseline-debt explain proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/1")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow list --allow-id allow-unsafe-baseline-debt --format json"),
        "unsafe baseline-debt list allow-id proof command"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/proof_commands/2")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --allow-id allow-unsafe-baseline-debt --format json"),
        "unsafe baseline-debt worklist allow-id proof command"
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_routes_unsafe_broken_evidence_closeout() {
    let fixture = SourceTreeFixture::new("saved-worklist-unsafe-broken-evidence-closeout");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-unsafe-broken-evidence.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "unsafe",
        "--item-kind",
        "broken_evidence_link",
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
        "unsafe broken-evidence closeout should contain one item"
    );
    assert_eq!(
        value
            .pointer("/filters/kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "worklist artifact should preserve the unsafe kind filter"
    );
    assert_eq!(
        value
            .pointer("/filters/item_kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link"),
        "worklist artifact should preserve the broken-evidence item-kind filter"
    );
    assert_eq!(
        value
            .pointer("/summary/item_kinds/broken_evidence_link")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "unsafe broken-evidence closeout summary should count the routed item"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link"),
        "unsafe broken-evidence worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/exception_kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "unsafe broken-evidence exception kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/family")
            .and_then(serde_json::Value::as_str),
        Some("unsafe_block"),
        "unsafe broken-evidence family"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/risk")
            .and_then(serde_json::Value::as_str),
        Some("high"),
        "unsafe broken-evidence risk"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "unsafe broken-evidence allow id"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/raw")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/missing-evidence.md"),
        "unsafe broken-evidence raw evidence reference"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("local_file_missing"),
        "unsafe broken-evidence status"
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow check --kind unsafe --mode no-new",
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow worklist --kind unsafe --format json",
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_routes_unsafe_weak_evidence_closeout() {
    let fixture = SourceTreeFixture::new("saved-worklist-unsafe-weak-evidence-closeout");
    fixture.write_policy_with_unsafe_baseline_debt_entry();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let worklist = artifact_dir.join("worklist-unsafe-weak-evidence.json");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "unsafe",
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
        "unsafe weak-evidence closeout should contain one item"
    );
    assert_eq!(
        value
            .pointer("/filters/kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "worklist artifact should preserve the unsafe kind filter"
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
            .pointer("/summary/item_kinds/weak_evidence_reference")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "unsafe weak-evidence closeout summary should count the routed item"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("weak_evidence_reference"),
        "unsafe weak-evidence worklist item kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/exception_kind")
            .and_then(serde_json::Value::as_str),
        Some("unsafe"),
        "unsafe weak-evidence exception kind"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/family")
            .and_then(serde_json::Value::as_str),
        Some("unsafe_block"),
        "unsafe weak-evidence family"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-unsafe-baseline-debt"),
        "unsafe weak-evidence allow id"
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
        Some("TODO: add unsafe-review or boundary-test evidence"),
        "unsafe weak-evidence raw evidence reference"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("unstructured"),
        "unsafe weak-evidence status"
    );
    assert_eq!(
        value
            .pointer("/work_items/0/suggested_actions/0")
            .and_then(serde_json::Value::as_str),
        Some(
            "replace weak evidence with unsafe-review, test, spec, or boundary evidence for the unsafe exception"
        ),
        "unsafe weak-evidence action should name the stronger evidence boundary"
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow check --kind unsafe --mode no-new",
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow worklist --kind unsafe --format json",
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

#[test]
fn saved_worklist_output_includes_redundant_segment_evidence_scope_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-redundant-evidence-scope");
    fixture.write_policy_with_redundant_segment_evidence_scope();

    let value = run_broken_evidence_worklist(&fixture);
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-redundant-segment-evidence-scope")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/path")
            .and_then(serde_json::Value::as_str),
        Some("docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/raw")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("invalid_local_path")
    );
    let message = value
        .pointer("/work_items/0/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("worklist item should include message"));
    assert!(
        message.contains("allow-redundant-segment-evidence-scope evidence `doc:docs/./safety.md`")
            && message.contains("current directory segments"),
        "worklist should explain why the redundant local evidence scope is invalid: {message}"
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow worklist --broken-evidence --format json",
    );
    assert_proof_commands_stay_cargo_allow(&value, "/work_items/0/proof_commands");
}

#[test]
fn saved_worklist_output_includes_redundant_segment_link_scope_items() {
    let fixture = SourceTreeFixture::new("saved-worklist-redundant-link-scope");
    fixture.write_policy_with_redundant_segment_link_scope();

    let value = run_broken_evidence_worklist(&fixture);
    assert_eq!(
        value
            .pointer("/summary/work_items")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/work_items/0/kind")
            .and_then(serde_json::Value::as_str),
        Some("broken_evidence_link")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/allow_id")
            .and_then(serde_json::Value::as_str),
        Some("allow-redundant-segment-link-scope")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/path")
            .and_then(serde_json::Value::as_str),
        Some("docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/raw")
            .and_then(serde_json::Value::as_str),
        Some("doc:docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/target")
            .and_then(serde_json::Value::as_str),
        Some("docs/./safety.md")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/evidence_reference/status")
            .and_then(serde_json::Value::as_str),
        Some("invalid_local_path")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/suggested_actions/0")
            .and_then(serde_json::Value::as_str),
        Some("restore or commit the referenced local traceability file")
    );
    assert_eq!(
        value
            .pointer("/work_items/0/suggested_actions/1")
            .and_then(serde_json::Value::as_str),
        Some("or update the link reference to a valid source-tree-relative path")
    );
    let message = value
        .pointer("/work_items/0/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("worklist item should include message"));
    assert!(
        message.contains("allow-redundant-segment-link-scope link `doc:docs/./safety.md`")
            && message.contains("link path must not contain current directory segments"),
        "worklist should explain why the redundant local traceability link scope is invalid: {message}"
    );
    assert_proof_command_present(
        &value,
        "/work_items/0/proof_commands",
        "cargo-allow worklist --broken-evidence --format json",
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
        "--broken-evidence",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist")
}

fn assert_proof_command_present(value: &serde_json::Value, pointer: &str, expected: &str) {
    let commands = value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{pointer} should point to proof_commands array")
        });
    assert!(
        commands
            .iter()
            .any(|command| command.as_str() == Some(expected)),
        "{pointer} should include `{expected}`: {commands:?}"
    );
}
