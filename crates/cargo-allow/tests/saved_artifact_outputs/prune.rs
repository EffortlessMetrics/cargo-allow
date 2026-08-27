use super::*;
use std::fs;

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
        .unwrap_or_else(|err| panic!("read pruned policy: {err}"));
    assert!(policy.contains("allow-panic-fixture"));
    assert!(!policy.contains("allow-stale-fixture"));
}
