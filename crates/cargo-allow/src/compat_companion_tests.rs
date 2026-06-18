use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, MatchStatus, Selector, Span,
    StructuralIdentity, normalize_path,
};
use allow_match::{CheckMode, evaluate};
use std::fs;
use std::path::PathBuf;

use crate::{
    canonical_companion_findings, compat_test_support::migrate_fixture_dir, extend_unique_findings,
};

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
    fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("workspace manifest write: {err}")));

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
    cfg.allow.push(companion_entry(
        "dep-cargo-toml",
        FindingKind::PolicyException,
        "dependency_surface",
        "Cargo.toml",
        "dependency_surface",
        "Cargo.toml",
        Some("workspace_manifest"),
    ));

    let inventory_files = vec![PathBuf::from("Cargo.toml")];
    let findings =
        canonical_companion_findings(&dir, &cfg, &inventory_files).unwrap_or_else(|err| {
            std::panic::panic_any(format!("canonical companion findings: {err}"))
        });
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

    assert_eq!(findings.len(), 6);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.status == MatchStatus::Matched),
        "expected every migrated companion entry to match current canonical findings: {outcomes:?}"
    );
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
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
        ledger: None,
    }
}
