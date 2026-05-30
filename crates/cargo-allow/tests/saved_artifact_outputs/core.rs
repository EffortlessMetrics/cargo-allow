use super::*;

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
