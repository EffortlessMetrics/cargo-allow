use std::{fs, path::Path};

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

#[test]
fn saved_migrate_output_preserves_legacy_evidence_and_links() {
    let fixture = SourceTreeFixture::new("saved-migrate-evidence-preserved");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create legacy policy dir: {err}")));
    fs::write(
        legacy_dir.join("generated-allowlist.toml"),
        generated_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write generated policy fixture: {err}")));
    fs::write(
        legacy_dir.join("executable-allowlist.toml"),
        executable_policy_with_covered_by_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write executable policy fixture: {err}")));

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
        "migrate evidence preservation allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(8),
        "migrate evidence preservation evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate evidence preservation link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate evidence preservation link count"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        ))
    });
    let generated = migrated_entry(&cfg, "saved-generated-evidence");
    assert_eq!(generated.kind, allow_core::FindingKind::GeneratedCode);
    assert_eq!(generated.family.as_deref(), Some("generated_code"));
    assert_eq!(
        generated.path.as_deref(),
        Some(Path::new("docs/generated/schema.json"))
    );
    assert_entry_evidence(
        generated,
        &[
            "doc:docs/generated/schema.md",
            "issue:#314",
            "legacy-policy:saved-generated-evidence",
            "generator:cargo xtask schema",
            "cargo:cargo xtask schema",
        ],
    );
    assert_entry_links(generated, &["legacy-policy:saved-generated-evidence"]);

    let executable = migrated_entry(&cfg, "saved-executable-covered");
    assert_eq!(executable.kind, allow_core::FindingKind::PolicyException);
    assert_eq!(executable.family.as_deref(), Some("executable_file"));
    assert_eq!(
        executable.path.as_deref(),
        Some(Path::new("scripts/package.sh"))
    );
    assert_entry_evidence(
        executable,
        &[
            "doc:docs/release/package.md",
            "legacy-policy:saved-executable-covered",
            "interpreter:bash",
        ],
    );
    assert_entry_links(executable, &["legacy-policy:saved-executable-covered"]);
}

#[test]
fn saved_migrate_output_routes_baseline_debt_follow_up() {
    let fixture = SourceTreeFixture::new("saved-migrate-baseline-debt-follow-up");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create legacy policy dir: {err}")));
    fs::write(
        legacy_dir.join("no-panic-baseline.toml"),
        no_panic_baseline_policy_fixture_text(),
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("write no-panic baseline policy fixture: {err}"))
    });

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
        Some(1),
        "migrate baseline-debt allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt summary count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "migrate baseline-debt unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate baseline-debt evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt link count"
    );
    let follow_up_queues = value
        .pointer("/follow_up_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route baseline-debt follow-up queues")
        });
    let [baseline_debt] = follow_up_queues.as_slice() else {
        std::panic::panic_any(format!(
            "expected one migrate baseline-debt follow-up queue, got {}",
            follow_up_queues.len()
        ));
    };
    assert_eq!(
        baseline_debt
            .get("signal")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue signal"
    );
    assert_eq!(
        baseline_debt
            .get("route_kind")
            .and_then(serde_json::Value::as_str),
        Some("worklist_item_kind"),
        "migrate baseline-debt queue route kind"
    );
    assert_eq!(
        baseline_debt
            .get("item_kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue item kind"
    );
    assert_eq!(
        baseline_debt
            .get("count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt queue count"
    );
    assert_eq!(
        baseline_debt
            .get("command")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind baseline_debt --format json"),
        "migrate baseline-debt queue command"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        ))
    });
    let baseline = migrated_entry(&cfg, "panic-baseline-0001");
    assert_eq!(baseline.kind, allow_core::FindingKind::Panic);
    assert_eq!(baseline.family.as_deref(), Some("unwrap"));
    assert_eq!(baseline.owner, "unowned");
    assert_eq!(baseline.classification, "baseline_debt");
    assert_eq!(baseline.occurrence_limit, Some(2));
    assert_entry_evidence(
        baseline,
        &[
            "legacy_policy:no-panic-baseline",
            "legacy_selector_callee:Option/Result::unwrap",
            "baseline_count:2",
        ],
    );
    assert_entry_links(baseline, &["legacy-policy:no-panic-baseline"]);
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

fn no_panic_baseline_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[policy_config]
mode = "no-new-debt"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "let value = maybe.unwrap();"
count = 2
"#
}

fn generated_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-generated-evidence"
path = "docs/generated/schema.json"
generator = "cargo xtask schema"
regenerate_command = "cargo xtask schema"
owner = "policy"
reason = "Generated schema fixture."
evidence = ["doc:docs/generated/schema.md", "issue:#314"]
created = "2026-05-10"
expires = "permanent"
"#
}

fn executable_policy_with_covered_by_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-executable-covered"
path = "scripts/package.sh"
interpreter = "bash"
owner = "release"
reason = "Package helper fixture."
covered_by = "doc:docs/release/package.md"
created = "2026-05-09"
expires = "permanent"
"#
}

fn migrated_entry<'a>(cfg: &'a allow_core::AllowConfig, id: &str) -> &'a allow_core::AllowEntry {
    cfg.allow
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing migrated entry {id}")))
}

fn assert_entry_evidence(entry: &allow_core::AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.evidence.iter().any(|item| item == value),
            "{} should preserve evidence `{value}` in {:?}",
            entry.id,
            entry.evidence
        );
    }
}

fn assert_entry_links(entry: &allow_core::AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.links.iter().any(|item| item == value),
            "{} should preserve link `{value}` in {:?}",
            entry.id,
            entry.links
        );
    }
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
