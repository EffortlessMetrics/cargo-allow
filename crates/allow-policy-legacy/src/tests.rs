use super::*;
use crate::findings::{dependency_surface_finding, workflow_action_finding, workflow_file_finding};
use crate::test_support::*;
use allow_core::FindingKind;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn migrates_non_rust_allowlist_to_canonical_policy() {
    let policy = policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 4);
    let docs = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected docs allow entry"));
    assert_eq!(docs.id, "non-rust-docs");
    assert_eq!(docs.glob.as_deref(), Some("docs/**"));
    assert_eq!(docs.lifecycle.expires.as_deref(), Some("never"));
    assert!(docs.reason.contains("Scope note:"));
    let ripr = cfg
        .allow
        .get(3)
        .unwrap_or_else(|| std::panic::panic_any("expected ripr allow entry"));
    assert_eq!(ripr.path.as_deref(), Some(Path::new("ripr.toml")));
    assert_eq!(ripr.selector.glob.as_deref(), Some("ripr.toml"));
}

#[test]
fn compat_config_expands_matching_findings_to_exact_entries() {
    let findings = vec![
        finding(".github/workflows/ci.yml", "tracked_file"),
        finding("unmatched/tool.py", "tracked_file"),
    ];

    let policy = policy_fixture_path();
    let cfg = load_non_rust_compat_config(&policy, &findings)
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy compat config loads: {err}")));

    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one compat allow entry"));
    assert_eq!(
        entry.path.as_deref(),
        Some(Path::new(".github/workflows/ci.yml"))
    );
    assert_eq!(entry.owner, "release/ci");
    assert_eq!(entry.classification, "ci_declarative");
    assert_eq!(
        entry.selector.glob.as_deref(),
        Some(".github/workflows/ci.yml")
    );
    assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
}

#[test]
fn compat_prefers_more_specific_rule_when_legacy_globs_overlap() {
    let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

    let policy = policy_fixture_path();
    let cfg = load_non_rust_compat_config(&policy, &findings)
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy compat config loads: {err}")));

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one compat allow entry"));
    assert_eq!(entry.owner, "release/ci");
    assert_eq!(entry.classification, "ci_declarative");
}

#[test]
fn non_rust_migration_rejects_broad_glob_without_reason() {
    let policy = non_rust_policy_with_entry(
        r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
created = "2026-05-09"
expires = "permanent"
"#,
    );

    let err = load_legacy_or_canonical(&policy)
        .expect_err("broad non-rust glob without reason should fail");

    assert!(err.to_string().contains("requires broad_glob_reason"));
}

#[test]
fn non_rust_migration_rejects_empty_broad_glob_reason() {
    let policy = non_rust_policy_with_entry(
        r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "   "
created = "2026-05-09"
expires = "permanent"
"#,
    );

    let err = load_legacy_or_canonical(&policy)
        .expect_err("empty broad non-rust glob reason should fail");

    assert!(err.to_string().contains("empty broad_glob_reason"));
}

#[test]
fn migrates_workflow_allowlist_to_policy_exception_entries() {
    let policy = workflow_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 3);
    assert!(cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::PolicyException
            && entry.family.as_deref() == Some("github_workflow")
            && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml"))
    }));
    let action = cfg
        .allow
        .iter()
        .find(|entry| {
            entry.family.as_deref() == Some("workflow_external_action")
                && entry
                    .selector
                    .target_fingerprint
                    .as_deref()
                    .is_some_and(|target| target == "action:actions/checkout@v6.0.2")
        })
        .unwrap_or_else(|| std::panic::panic_any("expected checkout action entry"));
    assert_eq!(action.classification, "workflow_external_action");
    assert_eq!(action.lifecycle.expires.as_deref(), Some("never"));
}

#[test]
fn workflow_findings_read_workflow_files_and_uses_lines() {
    let root = workflow_fixture_root();

    let findings = workflow_findings_from_files(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow findings load: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.family.as_deref() == Some("github_workflow")
            && finding.path == Path::new(".github/workflows/ci.yml")
    }));
    assert!(findings.iter().any(|finding| {
        finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref()
                == Some("action:actions/checkout@v6.0.2")
    }));
    assert!(!findings.iter().any(|finding| {
        finding.identity.target_fingerprint.as_deref() == Some("action:ignored/comment@v1")
    }));
}

#[test]
fn workflow_compat_preserves_missing_and_stale_drift() {
    let policy = workflow_policy_fixture_path();
    let cfg = load_workflow_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("workflow compat config loads: {err}"))
    });

    let matched = allow_match::evaluate(
        &cfg,
        &[
            workflow_file_finding(PathBuf::from(".github/workflows/ci.yml")),
            workflow_action_finding(
                PathBuf::from(".github/workflows/ci.yml"),
                "actions/checkout@v6.0.2".to_string(),
            ),
            workflow_action_finding(
                PathBuf::from(".github/workflows/ci.yml"),
                "Swatinem/rust-cache@v2".to_string(),
            ),
        ],
        allow_match::CheckMode::NoNew,
    );
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        3
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[
            workflow_file_finding(PathBuf::from(".github/workflows/ci.yml")),
            workflow_action_finding(
                PathBuf::from(".github/workflows/ci.yml"),
                "actions/setup-node@v5".to_string(),
            ),
        ],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        missing_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::New)
    );

    let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
    assert!(
        stale_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
    );
}

#[test]
fn migrates_dependency_surface_allowlist_to_policy_exception_entries() {
    let policy = dependency_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("dependency policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let workspace = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "dep-workspace-cargo-toml")
        .unwrap_or_else(|| std::panic::panic_any("expected workspace manifest entry"));
    assert_eq!(workspace.kind, FindingKind::PolicyException);
    assert_eq!(workspace.family.as_deref(), Some("dependency_surface"));
    assert_eq!(workspace.classification, "workspace_manifest");
    assert_eq!(workspace.path.as_deref(), Some(Path::new("Cargo.toml")));
    assert_eq!(workspace.lifecycle.expires.as_deref(), Some("never"));
    assert!(
        workspace
            .evidence
            .iter()
            .any(|item| item == "dep_count_at_baseline:22")
    );

    let crates = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "dep-crate-cargo-toml")
        .unwrap_or_else(|| std::panic::panic_any("expected crate glob entry"));
    assert_eq!(crates.glob.as_deref(), Some("crates/*/Cargo.toml"));
    assert!(crates.reason.contains("Scope note:"));
}

#[test]
fn dependency_surface_compat_preserves_matched_new_and_stale_drift() {
    let policy = dependency_policy_fixture_path();
    let cfg = load_dependency_surface_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("dependency compat config loads: {err}"))
    });

    let matched = allow_match::evaluate(
        &cfg,
        &[
            dependency_surface_finding(PathBuf::from("Cargo.toml")),
            dependency_surface_finding(PathBuf::from("crates/core/Cargo.toml")),
        ],
        allow_match::CheckMode::NoNew,
    );
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[dependency_surface_finding(PathBuf::from(
            "xtask/Cargo.toml",
        ))],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        missing_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::New)
    );

    let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
    assert!(
        stale_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
    );
}

#[test]
fn migrates_legacy_policy_directory_to_one_config() {
    let dir = fixture_dir();
    fs::write(
        dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));

    let cfg = load_legacy_policy_dir(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy directory migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.owner.as_deref(), Some("EffortlessMetrics"));
    assert_eq!(cfg.allow.len(), 4);
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.family.as_deref() == Some("process_spawn"))
    );
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.family.as_deref() == Some("network_destination"))
    );
}

#[test]
fn policy_directory_can_expand_non_rust_globs_with_findings() {
    let dir = fixture_dir();
    fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust fixture write: {err}")));
    let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

    let cfg =
        load_legacy_policy_dir_with_non_rust_findings(&dir, &findings).unwrap_or_else(|err| {
            std::panic::panic_any(format!("policy directory with findings migrates: {err}"))
        });

    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected expanded non-rust entry"));
    assert_eq!(entry.id, "non-rust-github-workflows--0001");
    assert_eq!(
        entry.path.as_deref(),
        Some(Path::new(".github/workflows/ci.yml"))
    );
    assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
}

#[test]
fn legacy_policy_directory_requires_supported_files() {
    let dir = fixture_dir();
    let err = load_legacy_policy_dir(&dir).expect_err("empty policy directory should not migrate");
    assert!(
        err.to_string()
            .contains("contains no supported legacy policy files")
    );
}
