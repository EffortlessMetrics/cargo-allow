use allow_inventory::InventorySource;
use std::fs;
use std::path::Path;

use crate::{compat_test_support::migrate_fixture_dir, load_compat_world};

#[test]
fn dependency_surface_compat_reports_git_source_without_inventory_count() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    let crate_dir = dir.join("crates").join("core");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&crate_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("crate dir: {err}")));
    fs::write(
        policy_dir.join("dependency-surface-allowlist.toml"),
        dependency_surface_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("dependency policy write: {err}")));
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/core\"]\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("workspace manifest: {err}")));
    fs::write(crate_dir.join("Cargo.toml"), "[package]\nname = \"core\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("crate manifest: {err}")));
    run_git_for_test(&dir, &["init"]);
    run_git_for_test(&dir, &["add", "Cargo.toml", "crates/core/Cargo.toml"]);

    let (_root, _cfg, findings, inventory_facts) =
        load_compat_world(Some(&dir), None, Some("dependency-surface"), false).unwrap_or_else(
            |err| std::panic::panic_any(format!("dependency compat world loads: {err}")),
        );

    assert_eq!(inventory_facts.source, InventorySource::GitTracked);
    assert_eq!(inventory_facts.files_scanned, None);
    assert_eq!(findings.len(), 2);
}

fn dependency_surface_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "dep-workspace-cargo-toml"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block."
created = "2026-05-09"
expires = "permanent"

[[allow]]
id = "dep-crate-cargo-toml"
path = "crates/*/Cargo.toml"
surface = "crate_manifest"
owner = "release"
reason = "Per-crate manifests."
broad_glob_reason = "Per-crate enumeration would duplicate the workspace member list."
created = "2026-05-09"
expires = "permanent"
"#
}

fn run_git_for_test(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    assert!(status.success(), "git {args:?} failed with {status}");
}
