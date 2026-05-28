use allow_core::{FindingKind, MatchStatus};
use allow_inventory::InventorySource;
use allow_match::{CheckMode, evaluate};
use std::fs;

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
