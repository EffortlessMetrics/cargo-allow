mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn saved_json_outputs_keep_source_tree_contracts() {
    let fixture = SourceTreeFixture::new("saved-json-contracts");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let audit = artifact_dir.join("audit.json");
    let check = artifact_dir.join("check.json");
    let receipt = artifact_dir.join("check.receipt.json");
    let list = artifact_dir.join("list.json");
    let worklist = artifact_dir.join("worklist.json");
    let doctor = artifact_dir.join("doctor.json");
    let explain = artifact_dir.join("explain.json");
    let prune = artifact_dir.join("prune.json");

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
    assert_source_syntax_artifact(&audit, allow_report::REPORT_SCHEMA_ID, "audit");

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
        path_arg(&check),
        "--receipt",
        path_arg(&receipt),
    ]);
    assert_source_syntax_artifact(&check, allow_report::REPORT_SCHEMA_ID, "check");
    assert_source_syntax_artifact(&receipt, allow_report::RECEIPT_SCHEMA_ID, "check");

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
    assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");

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
    assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");

    run_cargo_allow(&[
        "explain",
        "allow-panic-fixture",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&explain),
    ]);
    assert_source_syntax_artifact(&explain, allow_report::EXPLAIN_SCHEMA_ID, "explain");

    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--format",
        "json",
        "--output",
        path_arg(&prune),
    ]);
    let prune_value = assert_source_syntax_artifact(&prune, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        prune_value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "prune saved artifact should keep stale cleanup summary shape"
    );

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
    assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
}

#[test]
fn saved_summary_outputs_keep_policy_and_summary_streams_separate() {
    let fixture = SourceTreeFixture::new("saved-summary-contracts");
    fixture.write_minimal_policy();
    fixture.write_panic_source();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let add_policy = artifact_dir.join("allow.add.toml");
    let add_summary = artifact_dir.join("add-summary.json");
    let propose_policy = artifact_dir.join("allow.proposed.toml");
    let propose_summary = artifact_dir.join("propose-summary.json");
    let migrate_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

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
        "Fixture exercises saved add summary output.",
        "--evidence",
        "test:saved_summary_outputs_keep_policy_and_summary_streams_separate",
        "--write",
        path_arg(&add_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&add_summary),
    ]);
    assert_policy_output(&add_policy);
    assert_source_syntax_artifact(&add_summary, allow_report::ADD_SCHEMA_ID, "add");

    run_cargo_allow(&[
        "propose",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--write",
        path_arg(&propose_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&propose_summary),
    ]);
    assert_policy_output(&propose_policy);
    assert_source_syntax_artifact(&propose_summary, allow_report::PROPOSE_SCHEMA_ID, "propose");

    run_cargo_allow(&[
        "migrate",
        "--from",
        path_arg(&fixture.root.join("policy/allow.toml")),
        "--out",
        path_arg(&migrate_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);
    assert_policy_output(&migrate_policy);
    assert_policy_migration_artifact(&migrate_summary, allow_report::MIGRATE_SCHEMA_ID, "migrate");
}

#[test]
fn saved_diff_output_keeps_source_tree_contract() {
    let fixture = SourceTreeFixture::new("saved-diff-contract");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();
    commit_fixture_base(&fixture.root);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let diff = artifact_dir.join("diff.json");

    run_cargo_allow(&[
        "diff",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--base",
        "HEAD",
        "--format",
        "json",
        "--output",
        path_arg(&diff),
    ]);
    let value = assert_source_syntax_artifact_with_inventory(
        &diff,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        "git_tracked",
    );
    assert_eq!(
        value
            .pointer("/diff/net_posture")
            .and_then(serde_json::Value::as_str),
        Some("unchanged"),
        "saved diff artifact should include unchanged PR posture"
    );
}

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
            .pointer("/work_items/0/proof_commands/1")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --allow-id allow-broken-evidence --format json"),
        "worklist allow-id proof command"
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
        Some("cargo-allow worklist --allow-id allow-missing-evidence --format json"),
        "worklist allow-id proof command"
    );
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
        Some("cargo-allow worklist --allow-id allow-baseline-debt --format json"),
        "worklist allow-id proof command"
    );
}

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
fn saved_prune_output_allows_broken_evidence_preview() {
    let fixture = SourceTreeFixture::new("saved-prune-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let prune = artifact_dir.join("prune.json");

    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--format",
        "json",
        "--output",
        path_arg(&prune),
    ]);
    let value = assert_source_syntax_artifact(&prune, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "prune dry-run should still preview stale broken-evidence entries"
    );
    assert_eq!(
        value
            .pointer("/stale_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "prune should include the stale broken-evidence allow entry"
    );
    assert_eq!(
        value
            .pointer("/mode/dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "prune should remain dry-run first"
    );
}

#[test]
fn saved_prune_write_output_records_written_policy() {
    let fixture = SourceTreeFixture::new("saved-prune-write-output");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let prune = artifact_dir.join("prune-write.json");

    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--write",
        "--format",
        "json",
        "--output",
        path_arg(&prune),
    ]);
    let value = assert_source_syntax_artifact(&prune, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        value
            .pointer("/mode/dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "prune write artifact should not report dry-run mode"
    );
    assert_eq!(
        value
            .pointer("/mode/write_requested")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "prune write artifact should record write mode"
    );
    let written_path = value
        .pointer("/mode/written_path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("prune write artifact should include path"));
    assert!(
        written_path.ends_with("policy\\allow.toml") || written_path.ends_with("policy/allow.toml"),
        "prune written_path should identify policy/allow.toml: {written_path}"
    );
    assert_eq!(
        value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "prune write artifact should preserve the stale-entry count"
    );
    let policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read pruned policy: {err}")));
    assert!(policy.contains("allow-panic-fixture"));
    assert!(!policy.contains("allow-stale-fixture"));
}

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
        Some("../outside.md"),
        "worklist should expose the invalid source-tree-relative evidence target"
    );
    let message = value
        .pointer("/work_items/0/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("worklist item should include message"));
    assert!(
        message.contains("parent directory segments"),
        "worklist should explain why the local evidence scope is invalid: {message}"
    );
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

fn run_cargo_allow(args: &[&str]) -> Output {
    let output = cargo_allow_command()
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
    let command = format!("cargo-allow {}", args.join(" "));
    assert_status(&command, &output, true);
    assert_stdout_empty(
        &command,
        &output,
        "should not write stdout when output files are set",
    );
    assert_stderr_empty(
        &command,
        &output,
        "should not write stderr when output files are set",
    );
    output
}

fn assert_source_syntax_artifact(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
) -> serde_json::Value {
    assert_source_syntax_artifact_with_inventory(
        path,
        expected_schema_id,
        expected_command,
        "filesystem_fallback",
    )
}

fn assert_source_syntax_artifact_with_inventory(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
    expected_source: &str,
) -> serde_json::Value {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        expected_source,
    );
    value
}

fn assert_policy_migration_artifact(path: &Path, expected_schema_id: &str, expected_command: &str) {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_POLICY_MIGRATION,
        allow_report::INVENTORY_SOURCE_UNKNOWN,
    );
}

type Output = std::process::Output;

fn assert_inventory(value: &serde_json::Value, expected_scanner: &str, expected_source: &str) {
    assert_eq!(
        value
            .pointer("/inventory/scanner")
            .and_then(serde_json::Value::as_str),
        Some(expected_scanner),
        "inventory scanner"
    );
    assert_eq!(
        value
            .pointer("/inventory/source")
            .and_then(serde_json::Value::as_str),
        Some(expected_source),
        "inventory source"
    );
}

fn assert_policy_output(path: &Path) {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read policy output {}: {err}", path.display()))
    });
    assert!(
        text.contains("schema_version = \"0.1\""),
        "{} should be policy TOML",
        path.display()
    );
    assert!(
        !text.contains("\"schema_id\""),
        "{} should not contain summary JSON",
        path.display()
    );
}

fn path_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| std::panic::panic_any(format!("non-UTF-8 path: {}", path.display())))
}

fn commit_fixture_base(root: &Path) {
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

struct SourceTreeFixture {
    root: PathBuf,
    root_arg: String,
}

impl SourceTreeFixture {
    fn new(prefix: &str) -> Self {
        let root = temp_root(prefix);
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
        let root_arg = root
            .to_str()
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("non-UTF-8 fixture path: {}", root.display()))
            })
            .to_string();
        Self { root, root_arg }
    }

    fn root_str(&self) -> &str {
        &self.root_arg
    }

    fn write_minimal_policy(&self) {
        fs::write(
            self.root.join("policy/allow.toml"),
            r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_panic_source(&self) {
        fs::create_dir_all(self.root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        fs::write(
            self.root.join("src/lib.rs"),
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    }

    fn append_saved_artifact_allow_entries(&self) {
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-panic-fixture"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps explain/check saved artifact output covered."
evidence = ["test:saved_json_outputs_keep_source_tree_contracts"]
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"

[[allow]]
id = "allow-stale-fixture"
kind = "non_rust_file"
family = "documentation"
path = "docs/missing.md"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps prune saved artifact output covered."
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/missing.md"
target_fingerprint = "md"
glob = "docs/missing.md"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_policy_with_broken_evidence(&self) {
        self.write_policy_with_evidence(
            "allow-broken-evidence",
            "Fixture exercises broken evidence worklist output.",
            "doc:docs/missing-evidence.md",
        );
    }

    fn write_policy_with_invalid_evidence_scope(&self) {
        self.write_policy_with_evidence(
            "allow-invalid-evidence-scope",
            "Fixture exercises invalid evidence scope worklist output.",
            "doc:../outside.md",
        );
    }

    fn write_policy_with_missing_evidence_entry(&self) {
        self.write_minimal_policy();
        fs::create_dir_all(self.root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(
            self.root.join("docs/policy.md"),
            "# Policy\n\nFixture documentation surface.\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write docs fixture: {err}")));
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-missing-evidence"
kind = "non_rust_file"
family = "documentation"
path = "docs/policy.md"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps missing-evidence worklist saved artifact output covered."
created = "2026-05-29"
review_after = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/policy.md"
target_fingerprint = "md"
glob = "docs/policy.md"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_policy_with_baseline_debt_entry(&self) {
        self.write_minimal_policy();
        self.write_panic_source();
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-baseline-debt"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "unowned"
classification = "baseline_debt"
reason = "Generated by cargo-allow propose; requires human review."
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_policy_with_weak_evidence(&self) {
        self.write_policy_with_evidence(
            "allow-weak-evidence",
            "Fixture exercises weak evidence worklist output.",
            "spreadsheet:manual-review",
        );
    }

    fn write_policy_with_present_and_traceability_evidence(&self) {
        self.write_minimal_policy();
        fs::create_dir_all(self.root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        fs::write(
            self.root.join("src/lib.rs"),
            "pub fn load(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe source: {err}")));
        fs::create_dir_all(self.root.join("docs/evidence"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create evidence dir: {err}")));
        fs::write(
            self.root.join("docs/evidence/safety.md"),
            "# Safety evidence\n\nFixture evidence artifact.\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence fixture: {err}")));
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(
            r#"

[[allow]]
id = "allow-evidence-diagnostics"
kind = "unsafe"
family = "unsafe_block"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "Fixture keeps explain evidence diagnostics covered."
evidence = [
  "doc:docs/evidence/safety.md",
  "test:saved_explain_output_reports_present_and_traceability_evidence",
]
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "unsafe_block"
"#,
        );
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_policy_with_evidence(&self, id: &str, reason: &str, evidence: &str) {
        self.write_minimal_policy();
        let mut policy = fs::read_to_string(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
        policy.push_str(&format!(
            r#"

[[allow]]
id = "{id}"
kind = "unsafe"
family = "unsafe_block"
path = "src/lib.rs"
owner = "core/tests"
classification = "reviewed_fixture"
reason = "{reason}"
evidence = ["{evidence}"]
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "unsafe_block"
"#,
        ));
        fs::write(self.root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }
}

impl Drop for SourceTreeFixture {
    fn drop(&mut self) {
        remove_temp_root(self.root.clone());
    }
}
