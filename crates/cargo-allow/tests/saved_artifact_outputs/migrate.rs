use std::fs;

use super::*;

#[test]
fn saved_migrate_output_covers_policy_migration_summary_contract() {
    let fixture = SourceTreeFixture::new("saved-migrate-summary");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create legacy policy dir: {err}")));
    fs::write(
        legacy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write process policy fixture: {err}")));
    fs::write(
        legacy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe policy fixture: {err}")));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate summary allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(7),
        "migrate summary evidence reference count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe weak evidence count"
    );
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route evidence repair queues")
        });
    assert!(
        queues.iter().any(|queue| {
            queue
                .get("unsafe_command")
                .and_then(serde_json::Value::as_str)
                == Some(
                    "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json",
                )
        }),
        "migrate summary should route unsafe broken evidence repair"
    );
    assert!(
        queues.iter().any(|queue| {
            queue
                .get("unsafe_command")
                .and_then(serde_json::Value::as_str)
                == Some(
                    "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json",
                )
        }),
        "migrate summary should route unsafe weak evidence repair"
    );
    let actual_policy_output = value
        .pointer("/output/path")
        .and_then(serde_json::Value::as_str)
        .map(|path| path.replace('\\', "/"));
    let expected_policy_output = path_arg(&migrated_policy).replace('\\', "/");
    assert_eq!(
        actual_policy_output.as_deref(),
        Some(expected_policy_output.as_str()),
        "migrate policy output path"
    );
}

fn process_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-bash-package-proof"
binary = "bash"
argv_shape = ["scripts/package-proof.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release preflight package proof; pure local checks."
created = "2026-05-09"
expires = "permanent"
"#
}

fn unsafe_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-ffi-boundary"
path = "src/lib.rs"
family = "unsafe_fn"
kind = "unsafe-fn"
evidence = [
  "doc:docs/safety/missing-ffi.md",
  "TODO: add unsafe-review or boundary-test evidence"
]

[allow.selector]
kind = "unsafe-fn"
"#
}
