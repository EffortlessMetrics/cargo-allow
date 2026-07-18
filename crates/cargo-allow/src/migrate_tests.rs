use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn clap_parses_repo_policy_migrate() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "migrate",
        "--repo-policy",
        "policy",
        "--out",
        "target/allow.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/migrate-summary.json",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse repo-policy migrate: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Migrate(MigrateArgs {
            repo_policy: Some(dir),
            out,
            force: true,
            summary_format: MigrateSummaryFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if dir == Path::new("policy")
            && out == Path::new("target/allow.toml")
            && summary_output == Path::new("target/migrate-summary.json")
    ));
}

#[test]
fn migrate_requires_one_input_source() {
    let missing = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: None,
        out: PathBuf::from("target/unused.toml"),
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Human,
        summary_output: None,
    })
    .expect_err("missing input source should fail");
    assert!(
        missing
            .to_string()
            .contains("pass --from <file> or --repo-policy <dir>")
    );

    let conflicting = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: Some(PathBuf::from("legacy.toml")),
        repo_policy: Some(PathBuf::from("policy")),
        out: PathBuf::from("target/unused.toml"),
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Human,
        summary_output: None,
    })
    .expect_err("conflicting input sources should fail");
    assert!(
        conflicting
            .to_string()
            .contains("pass either --from or --repo-policy")
    );
}

#[test]
fn migrate_refuses_existing_output_without_force() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");
    fs::write(&out, "existing")
        .unwrap_or_else(|err| std::panic::panic_any(format!("existing output write: {err}")));

    let err = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Human,
        summary_output: None,
    })
    .expect_err("existing output should require --force");
    assert!(err.to_string().contains("use --force to overwrite"));
}

#[test]
fn migrate_repo_policy_writes_combined_canonical_policy() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

    let rendered = fs::read_to_string(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrated policy: {err}")));
    assert!(rendered.contains("process_spawn"));
    assert!(rendered.contains("network_destination"));
}

#[test]
fn migrate_repo_policy_writes_json_summary_with_inventory_context() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(allow_report::MIGRATE_SCHEMA_ID),
        "migrate schema id"
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("migrate"),
        "migrate command"
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "migrate inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("policy_migration"),
        "migrate inventory scanner"
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("filesystem_fallback"),
        "migrate inventory source"
    );
    assert!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "repo-policy migration summary should include source-tree inventory file count"
    );
    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("repo_policy"),
        "migrate input kind"
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(2),
        "migrate allow entries"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(2),
        "migrate evidence-bearing entries"
    );
    assert!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 2),
        "repo-policy migration summary should count migrated evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(Value::as_u64),
        Some(2),
        "repo-policy migration summary should count link-bearing migrated entries"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(Value::as_u64),
        Some(2),
        "repo-policy migration summary should count canonical traceability links"
    );
    assert!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "repo-policy migration summary should surface weak evidence references"
    );
}

#[test]
fn migrate_repo_policy_summary_counts_unsafe_weak_evidence() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_missing_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe entries"
    );
    assert_eq!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count weak evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe weak evidence references"
    );
}

#[test]
fn migrate_repo_policy_summary_counts_unsafe_broken_evidence() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_broken_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count broken local evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe broken local evidence references"
    );
    assert!(
        value.pointer("/summary/weak_evidence_references").is_none(),
        "typed missing local evidence should not be classified as weak evidence"
    );
    assert!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .is_none(),
        "typed missing local unsafe evidence should not be classified as weak evidence"
    );
}

#[test]
fn migrate_repo_policy_human_summary_routes_evidence_repair_queues() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_broken_and_weak_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.txt");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Human,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));

    assert!(summary.contains("broken_evidence_links: 1"));
    assert!(summary.contains("unsafe_broken_evidence_links: 1"));
    assert!(summary.contains("weak_evidence_references: 1"));
    assert!(summary.contains("unsafe_weak_evidence_references: 1"));
    assert!(summary.contains("evidence_repair_queues:"));
    assert!(
        summary.contains("cargo-allow worklist --item-kind broken_evidence_link --format json")
    );
    assert!(summary.contains(
        "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"
    ));
    assert!(
        summary.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(summary.contains(
        "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
    ));
}

#[test]
fn migrate_from_uses_explicit_root_for_evidence_diagnostics() {
    let dir = migrate_fixture_dir();
    let docs_dir = dir.join("docs/safety");
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(
        docs_dir.join("migrated-boundary.md"),
        "reviewed migration boundary",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("evidence write: {err}")));
    let from = dir.join("legacy.allow.toml");
    fs::write(&from, canonical_policy_with_present_evidence_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs {
            root: Some(dir.clone()),
        },
        from: Some(from),
        repo_policy: None,
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("single-file migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));
    let expected_root = allow_report::source_tree_path_text(&dir);

    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("from"),
        "single-file migrate input kind"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some(expected_root.as_str()),
        "single-file migrate should record explicit source-tree root"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(1),
        "single-file migrate evidence-bearing entries"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(Value::as_u64),
        Some(1),
        "single-file migrate evidence reference entries"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "present local evidence under --root should not be reported as broken"
    );
    assert!(
        value.pointer("/evidence_repair_queues").is_none(),
        "present local evidence under --root should not route repair work"
    );
}

#[test]
fn migrate_from_infers_root_for_evidence_diagnostics() {
    let dir = migrate_fixture_dir();
    let docs_dir = dir.join("docs/safety");
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(
        docs_dir.join("migrated-boundary.md"),
        "reviewed migration boundary",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("evidence write: {err}")));
    let from = dir.join("legacy.allow.toml");
    fs::write(&from, canonical_policy_with_present_evidence_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: Some(from),
        repo_policy: None,
        out,
        force: false,
        update: false,
        summary_format: MigrateSummaryFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("single-file migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));
    let expected_root = allow_report::source_tree_path_text(&dir);

    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("filesystem_fallback"),
        "single-file migrate should report inferred inventory source"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some(expected_root.as_str()),
        "single-file migrate should infer source-tree root from --from"
    );
    assert!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "single-file migrate should report inferred inventory file count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "present local evidence under inferred root should not be reported as broken"
    );
    assert!(
        value.pointer("/evidence_repair_queues").is_none(),
        "present local evidence under inferred root should not route repair work"
    );
}

fn migrate_fixture_dir() -> PathBuf {
    static NEXT_MIGRATE_FIXTURE: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_MIGRATE_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-migrate-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn canonical_policy_with_present_evidence_fixture_text() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "EffortlessMetrics"
status = "active"

[[allow]]
id = "allow-migrated-doc"
kind = "non_rust_file"
family = "documentation"
path = "README.md"
owner = "docs"
classification = "reviewed_documentation"
reason = "Retained documentation file carried forward from legacy migration."
created = "2026-06-02"
review_after = "2026-11-01"
evidence = ["doc:docs/safety/migrated-boundary.md"]

[allow.selector]
ast_kind = "tracked_file"
symbol = "README.md"
target_fingerprint = "md"
line_hint = 1
"#
}

fn unsafe_policy_missing_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
path = "src/lib.rs"
family = "unsafe_fn"

[allow.selector]
kind = "unsafe-fn"
"#
}

fn unsafe_policy_broken_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "legacy-unsafe-missing-doc"
path = "src/lib.rs"
family = "unsafe_fn"
kind = "unsafe-fn"
evidence = ["doc:docs/safety/missing-ffi.md"]

[allow.selector]
kind = "unsafe-fn"
"#
}

fn unsafe_policy_broken_and_weak_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "legacy-unsafe-missing-and-todo"
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

fn process_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"
"#
}

fn network_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
