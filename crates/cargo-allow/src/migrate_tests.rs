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
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "repo-policy migration summary should surface weak evidence references"
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
