use super::*;
use crate::findings::{
    dependency_surface_finding, dependency_surface_findings_from_paths, workflow_action_finding,
    workflow_file_finding, workflow_findings_from_sources,
};
use crate::test_support::*;
use allow_core::FindingKind;
use std::path::{Path, PathBuf};

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
    assert_eq!(action.lifecycle.review_after.as_deref(), Some("2026-05-09"));
    assert!(
        action
            .evidence
            .iter()
            .any(|item| item == "legacy-policy:workflow:.github/workflows/ci.yml")
    );
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
fn workflow_findings_can_use_source_tree_text() {
    let findings = workflow_findings_from_sources(vec![(
        PathBuf::from(".github/workflows/ci.yml"),
        "steps:\n  - uses: actions/checkout@v4\n  - uses: dtolnay/rust-toolchain@stable\n"
            .to_string(),
    )]);

    assert!(findings.iter().any(|finding| {
        finding.family.as_deref() == Some("github_workflow")
            && finding.path == Path::new(".github/workflows/ci.yml")
    }));
    assert!(findings.iter().any(|finding| {
        finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref() == Some("action:actions/checkout@v4")
    }));
    assert!(findings.iter().any(|finding| {
        finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref()
                == Some("action:dtolnay/rust-toolchain@stable")
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
            .filter(|outcome| {
                outcome.finding_index.is_some()
                    && matches!(
                        outcome.status,
                        allow_core::MatchStatus::Matched | allow_core::MatchStatus::ReviewDue
                    )
            })
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
    assert!(stale_allow.iter().any(|outcome| {
        outcome.finding_index.is_none()
            && matches!(
                outcome.status,
                allow_core::MatchStatus::Stale | allow_core::MatchStatus::ReviewDue
            )
    }));
}

#[test]
fn workflow_migration_preserves_legacy_evidence_when_present() {
    let path = fixture_dir().join("workflow-allowlist.toml");
    std::fs::write(
        &path,
        r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = ["contents:read"]
secrets_used = []
external_actions = ["actions/checkout@v4"]
evidence = ["doc:docs/ci.md", "issue:#123"]
created = "2026-05-09"
expires = "permanent"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_legacy_or_canonical(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("workflow policy with evidence migrates: {err}"))
    });

    let workflow = cfg
        .allow
        .iter()
        .find(|entry| entry.family.as_deref() == Some("github_workflow"))
        .unwrap_or_else(|| std::panic::panic_any("expected workflow file entry"));
    assert!(
        workflow
            .evidence
            .iter()
            .any(|item| item == "doc:docs/ci.md")
    );
    assert!(workflow.evidence.iter().any(|item| item == "issue:#123"));
    assert!(
        workflow
            .evidence
            .iter()
            .any(|item| item == "legacy-policy:workflow:.github/workflows/release.yml")
    );
    assert!(
        workflow
            .evidence
            .iter()
            .any(|item| item == "permission:contents:read")
    );

    let action = cfg
        .allow
        .iter()
        .find(|entry| entry.family.as_deref() == Some("workflow_external_action"))
        .unwrap_or_else(|| std::panic::panic_any("expected workflow action entry"));
    assert!(action.evidence.iter().any(|item| item == "doc:docs/ci.md"));
    assert!(
        action
            .evidence
            .iter()
            .any(|item| item == "legacy-policy:workflow:.github/workflows/release.yml")
    );
    assert!(
        action
            .evidence
            .iter()
            .any(|item| item == "external_action:actions/checkout@v4")
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
    assert_eq!(
        workspace.lifecycle.review_after.as_deref(),
        Some("2026-05-09")
    );
    assert!(
        workspace
            .evidence
            .iter()
            .any(|item| item == "legacy-policy:dep-workspace-cargo-toml")
    );
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
fn dependency_surface_migration_accepts_covered_by_as_legacy_evidence() {
    let path = fixture_dir().join("dependency-surface-allowlist.toml");
    std::fs::write(
        &path,
        r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "dep-release-manifest"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block fixture."
dep_count_at_baseline = 22
covered_by = "doc:docs/dependencies.md"
created = "2026-05-09"
expires = "permanent"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_legacy_or_canonical(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("dependency policy with covered_by migrates: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected dependency surface entry"));
    assert_eq!(
        entry.evidence,
        vec![
            "doc:docs/dependencies.md".to_string(),
            "legacy-policy:dep-release-manifest".to_string(),
            "surface:workspace_manifest".to_string(),
            "dep_count_at_baseline:22".to_string(),
        ]
    );
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
            .filter(|outcome| {
                outcome.finding_index.is_some()
                    && matches!(
                        outcome.status,
                        allow_core::MatchStatus::Matched | allow_core::MatchStatus::ReviewDue
                    )
            })
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
    assert!(stale_allow.iter().any(|outcome| {
        outcome.finding_index.is_none()
            && matches!(
                outcome.status,
                allow_core::MatchStatus::Stale | allow_core::MatchStatus::ReviewDue
            )
    }));
}

#[test]
fn dependency_surface_findings_can_use_source_tree_inventory_paths() {
    let policy = dependency_policy_fixture_path();
    let cfg = load_dependency_surface_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("dependency compat config loads: {err}"))
    });
    let paths = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("crates/core/Cargo.toml"),
        PathBuf::from("policy/dependency-surface-allowlist.toml"),
    ];

    let findings = dependency_surface_findings_from_paths(&paths, &cfg);

    assert_eq!(findings.len(), 2);
    assert!(
        findings
            .iter()
            .any(|finding| finding.path == Path::new("Cargo.toml"))
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.path == Path::new("crates/core/Cargo.toml"))
    );
}
