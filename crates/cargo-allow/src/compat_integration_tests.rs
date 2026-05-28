use allow_core::{FindingKind, MatchStatus};
use allow_inventory::InventorySource;
use allow_match::{CheckMode, evaluate};
use std::fs;
use std::path::Path;

use crate::{compat_test_support::migrate_fixture_dir, load_compat_world};

#[test]
fn panic_compat_loads_no_panic_baseline_and_scans_source_tree_findings() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    let src_dir = dir.join("src");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&src_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    let snippet = "let value = maybe.unwrap();";
    fs::write(
        policy_dir.join("no-panic-baseline.toml"),
        format!(
            r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "{snippet}"
count = 1
"#
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic policy write: {err}")));
    fs::write(
        src_dir.join("lib.rs"),
        format!("fn load(maybe: Option<u8>) {{\n    {snippet}\n}}\n"),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

    let (_root, cfg, findings, inventory_facts) =
        load_compat_world(Some(&dir), None, Some("panic"), false).unwrap_or_else(|err| {
            std::panic::panic_any(format!("panic compat world loads: {err}"))
        });
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
    assert!(inventory_facts.files_scanned.is_some());
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.classification == "baseline_debt"
                && entry.occurrence_limit == Some(1))
    );
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::Panic && finding.family.as_deref() == Some("unwrap")
    }));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn no_panic_allowlist_compat_loads_policy_and_scans_panic_findings() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    let src_dir = dir.join("src");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&src_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        policy_dir.join("no-panic-allowlist.toml"),
        r#"schema_version = 1
policy = "no-panic-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "no-panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "parser"
classification = "reviewed_panic_exception"
reason = "Parser validates the optional value."
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "Option/Result::unwrap"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic policy write: {err}")));
    fs::write(
        src_dir.join("lib.rs"),
        "fn load(maybe: Option<u8>) {\n    let value = maybe.unwrap();\n}\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

    let (_root, cfg, findings, inventory_facts) =
        load_compat_world(Some(&dir), None, Some("no-panic-allowlist"), false).unwrap_or_else(
            |err| std::panic::panic_any(format!("no-panic allowlist world loads: {err}")),
        );
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
    assert!(inventory_facts.files_scanned.is_some());
    assert!(cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::Panic && entry.selector.callee.as_deref() == Some("unwrap")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::Panic && finding.family.as_deref() == Some("unwrap")
    }));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn clippy_compat_loads_legacy_policy_and_scans_lint_findings() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    let src_dir = dir.join("src");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&src_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        policy_dir.join("clippy-exceptions.toml"),
        r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "clippy-unwrap-policy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Fixture keeps an explicit lint suppression linked to policy."
policy_id = "clippy-unwrap-policy"
created = "2026-05-09"
review_after = "2026-09-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("clippy policy write: {err}")));
    fs::write(
        src_dir.join("lib.rs"),
        r#"#[expect(clippy::unwrap_used, reason = "policy:clippy-unwrap-policy: fixture")]
fn load() {}
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

    let (_root, cfg, findings, inventory_facts) =
        load_compat_world(Some(&dir), None, Some("lint-exception"), false).unwrap_or_else(|err| {
            std::panic::panic_any(format!("clippy compat world loads: {err}"))
        });
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
    assert!(inventory_facts.files_scanned.is_some());
    assert!(cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::LintException
            && entry.selector.lint.as_deref() == Some("clippy::unwrap_used")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::LintException
            && finding.family.as_deref() == Some("expect_attribute")
    }));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn unsafe_compat_loads_legacy_policy_and_scans_unsafe_findings() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    let src_dir = dir.join("src");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&src_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe_block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates pointer before read."
evidence = ["unsafe-review:docs/evidence/unsafe/read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe policy write: {err}")));
    fs::write(
        src_dir.join("lib.rs"),
        "fn read(ptr: *const u8) -> u8 {\n    // SAFETY: fixture validates the policy match path.\n    unsafe { core::ptr::read(ptr) }\n}\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

    let (_root, cfg, findings, inventory_facts) =
        load_compat_world(Some(&dir), None, Some("unsafe"), false).unwrap_or_else(|err| {
            std::panic::panic_any(format!("unsafe compat world loads: {err}"))
        });
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
    assert!(inventory_facts.files_scanned.is_some());
    assert!(cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::Unsafe
            && entry.selector.ast_kind.as_deref() == Some("unsafe_block")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::Unsafe && finding.family.as_deref() == Some("unsafe_block")
    }));
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

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
