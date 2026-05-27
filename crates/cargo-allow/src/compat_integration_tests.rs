use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, MatchStatus, Selector, Span,
    StructuralIdentity, normalize_path,
};
use allow_inventory::InventorySource;
use allow_match::{CheckMode, evaluate};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{canonical_companion_findings, extend_unique_findings, load_compat_world};

#[test]
fn canonical_companion_findings_match_migrated_policy_entries() {
    let dir = migrate_fixture_dir();
    let workflows_dir = dir.join(".github").join("workflows");
    fs::create_dir_all(&workflows_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(
        dir.join(".gitattributes"),
        "generated/schema.json linguist-generated=true\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
    fs::write(
        workflows_dir.join("ci.yml"),
        "steps:\n  - uses: actions/checkout@v4\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(companion_entry(
        "generated-schema",
        FindingKind::GeneratedCode,
        "generated_code",
        "generated/schema.json",
        "tracked_file",
        "generated/schema.json",
        Some("json"),
    ));
    cfg.allow.push(companion_entry(
        "workflow-file-ci",
        FindingKind::PolicyException,
        "github_workflow",
        ".github/workflows/ci.yml",
        "github_workflow",
        ".github/workflows/ci.yml",
        None,
    ));
    cfg.allow.push(companion_entry(
        "workflow-action-ci-checkout",
        FindingKind::PolicyException,
        "workflow_external_action",
        ".github/workflows/ci.yml",
        "github_action_uses",
        ".github/workflows/ci.yml uses actions/checkout@v4",
        Some("action:actions/checkout@v4"),
    ));
    cfg.allow.push(companion_entry(
        "proc-cargo-test",
        FindingKind::PolicyException,
        "process_spawn",
        ".github/workflows/ci.yml",
        "process_spawn",
        "cargo test",
        Some("process:cargo test"),
    ));
    cfg.allow.push(companion_entry(
        "net-crates-io",
        FindingKind::PolicyException,
        "network_destination",
        "policy/network-allowlist.toml",
        "network_destination",
        "crates.io lane build",
        Some("network:crates.io:auth:false:lane:build"),
    ));

    let findings = canonical_companion_findings(&dir, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!("canonical companion findings: {err}"))
    });
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(findings.len(), 5);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.status == MatchStatus::Matched),
        "expected every migrated companion entry to match current canonical findings: {outcomes:?}"
    );
}

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

#[test]
fn extend_unique_findings_deduplicates_generated_companion_inventory() {
    let mut generated = test_finding(
        FindingKind::GeneratedCode,
        Some("generated_code"),
        "generated/schema.json",
        "tracked_file",
    );
    generated.identity.symbol = Some("generated/schema.json".to_string());
    generated.identity.target_fingerprint = Some("json".to_string());
    let duplicate = generated.clone();
    let mut existing = vec![generated];
    let distinct = test_finding(
        FindingKind::GeneratedCode,
        Some("generated_code"),
        "generated/other.json",
        "tracked_file",
    );

    extend_unique_findings(&mut existing, vec![duplicate, distinct]);

    assert_eq!(existing.len(), 2);
    assert!(
        existing
            .iter()
            .any(|finding| normalize_path(&finding.path) == "generated/other.json")
    );
}

static NEXT_MIGRATE_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn migrate_fixture_dir() -> PathBuf {
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

fn companion_entry(
    id: &str,
    kind: FindingKind,
    family: &str,
    path: &str,
    ast_kind: &str,
    symbol: &str,
    target_fingerprint: Option<&str>,
) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: Some(family.to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "owner".to_string(),
        classification: family.to_string(),
        reason: "retained migrated policy entry".to_string(),
        evidence: vec!["legacy-policy:test".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-11-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(ast_kind.to_string()),
            symbol: Some(symbol.to_string()),
            target_fingerprint: target_fingerprint.map(str::to_string),
            glob: Some(path.to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn test_finding(kind: FindingKind, family: Option<&str>, path: &str, ast_kind: &str) -> Finding {
    test_finding_at_line(kind, family, path, ast_kind, 1)
}

fn test_finding_at_line(
    kind: FindingKind,
    family: Option<&str>,
    path: &str,
    ast_kind: &str,
    line: u32,
) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
    }
}
