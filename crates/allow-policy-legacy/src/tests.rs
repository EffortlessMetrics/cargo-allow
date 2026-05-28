use super::*;
use crate::findings::{
    dependency_surface_finding, executable_finding, executable_findings_from_git_stage,
    generated_finding, workflow_action_finding, workflow_file_finding,
};
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
fn migrates_generated_allowlist_to_canonical_policy() {
    let policy = generated_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected generated allow entry"));
    assert_eq!(entry.kind, FindingKind::GeneratedCode);
    assert_eq!(entry.family.as_deref(), Some("generated_code"));
    assert_eq!(
        entry.path.as_deref(),
        Some(Path::new("policy/no-panic-baseline.toml"))
    );
    assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
    assert!(entry.evidence.iter().any(|item| item.starts_with("cargo:")));
}

#[test]
fn migrates_clippy_exceptions_to_lint_policy_entries() {
    let policy = clippy_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("clippy exceptions migrate: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
    assert_eq!(entry.id, "clippy-unwrap-policy");
    assert_eq!(entry.kind, FindingKind::LintException);
    assert_eq!(entry.family.as_deref(), Some("expect_attribute"));
    assert_eq!(entry.path.as_deref(), Some(Path::new("src/lib.rs")));
    assert_eq!(entry.owner, "lint");
    assert_eq!(entry.classification, "reviewed_lint_exception");
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("attribute"));
    assert_eq!(entry.selector.lint.as_deref(), Some("clippy::unwrap_used"));
    assert_eq!(
        entry.selector.target_fingerprint.as_deref(),
        Some("policy:clippy-unwrap-policy")
    );
    assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-09-09"));
}

#[test]
fn clippy_compat_preserves_matched_new_and_stale_drift() {
    let policy = clippy_policy_fixture_path();
    let cfg = load_clippy_exceptions_compat_config(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("clippy compat config loads: {err}")));

    let matched = allow_match::evaluate(
        &cfg,
        &[lint_finding(
            "src/lib.rs",
            "expect_attribute",
            "clippy::unwrap_used",
            Some("clippy-unwrap-policy"),
        )],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        matched
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Matched)
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[lint_finding(
            "src/lib.rs",
            "expect_attribute",
            "clippy::panic",
            None,
        )],
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
fn clippy_compat_accepts_minimal_legacy_entries_as_baseline_debt() {
    let path = fixture_dir().join("clippy-exceptions.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "clippy-exceptions"

[[allow]]
path = "src/lib.rs"
lint = "clippy::unwrap_used"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_clippy_exceptions_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("minimal clippy compat config loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
    assert_eq!(entry.owner, "unowned");
    assert_eq!(entry.classification, "baseline_debt");
    assert!(entry.reason.contains("requires human review"));
    assert_current_baseline_window(&entry.lifecycle);
}

#[test]
fn clippy_compat_loader_requires_clippy_policy() {
    let policy = generated_policy_fixture_path();

    let err = load_clippy_exceptions_compat_config(&policy)
        .expect_err("generated policy should not load as clippy compat");

    assert!(err.to_string().contains("not a clippy-exceptions policy"));
}

#[test]
fn migrates_unsafe_allowlist_to_structural_unsafe_entries() {
    let policy = unsafe_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe allowlist migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let reviewed = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "unsafe-read")
        .unwrap_or_else(|| std::panic::panic_any("expected reviewed unsafe entry"));
    assert_eq!(reviewed.kind, FindingKind::Unsafe);
    assert_eq!(reviewed.family.as_deref(), Some("unsafe_block"));
    assert_eq!(reviewed.selector.ast_kind.as_deref(), Some("unsafe_block"));
    assert_eq!(reviewed.selector.container.as_deref(), Some("read"));
    assert_eq!(reviewed.selector.line_hint, Some(7));
    assert_eq!(
        reviewed
            .last_seen
            .as_ref()
            .map(|seen| (seen.line, seen.column)),
        Some((7, 12))
    );
    assert!(
        reviewed
            .evidence
            .iter()
            .any(|item| item == "unsafe-review:docs/evidence/unsafe/read.json")
    );

    let generated = cfg
        .allow
        .iter()
        .find(|entry| entry.id.starts_with("legacy-unsafe-"))
        .unwrap_or_else(|| std::panic::panic_any("expected generated unsafe entry"));
    assert_eq!(generated.family.as_deref(), Some("unsafe_fn"));
    assert_eq!(generated.classification, "baseline_debt");
    assert_eq!(generated.owner, "unowned");
    assert!(
        generated
            .evidence
            .iter()
            .any(|item| item.contains("TODO: add unsafe-review"))
    );
    assert_current_baseline_window(&generated.lifecycle);
}

#[test]
fn unsafe_allowlist_compat_preserves_matched_new_and_stale_drift() {
    let policy = unsafe_policy_fixture_path();
    let cfg = load_unsafe_allowlist_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("unsafe allowlist compat config loads: {err}"))
    });

    let matched = allow_match::evaluate(
        &cfg,
        &[unsafe_finding("src/lib.rs", "unsafe_block", Some("read"))],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        matched
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Matched)
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[unsafe_finding("src/lib.rs", "unsafe_impl", None)],
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
fn unsafe_allowlist_loader_requires_unsafe_policy() {
    let policy = generated_policy_fixture_path();

    let err = load_unsafe_allowlist_compat_config(&policy)
        .expect_err("generated policy should not load as unsafe compat");

    assert!(err.to_string().contains("not an unsafe-allowlist policy"));
}

#[test]
fn generated_findings_read_linguist_generated_paths() {
    let root = generated_fixture_root();

    let findings = generated_findings_from_gitattributes(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated findings load: {err}")));

    assert_eq!(findings.len(), 1);
    let finding = findings
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected generated finding"));
    assert_eq!(finding.kind, FindingKind::GeneratedCode);
    assert_eq!(finding.path, PathBuf::from("policy/no-panic-baseline.toml"));
}

#[test]
fn generated_compat_preserves_missing_and_stale_drift() {
    let policy = generated_policy_fixture_path();
    let cfg = load_generated_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("generated compat config loads: {err}"))
    });

    let matched = allow_match::evaluate(
        &cfg,
        &[generated_finding(PathBuf::from(
            "policy/no-panic-baseline.toml",
        ))],
        allow_match::CheckMode::NoNew,
    );
    assert!(matched.iter().any(|outcome| {
        outcome.status == allow_core::MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("generated-no-panic-baseline")
    }));

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[generated_finding(PathBuf::from(
            "policy/extra-baseline.toml",
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
fn migrates_executable_allowlist_to_policy_exception_entries() {
    let policy = executable_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("executable policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected executable allow entry"));
    assert_eq!(entry.kind, FindingKind::PolicyException);
    assert_eq!(entry.family.as_deref(), Some("executable_file"));
    assert_eq!(entry.classification, "executable_file");
    assert_eq!(
        entry.path.as_deref(),
        Some(Path::new("scripts/package-proof.sh"))
    );
    assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
    assert_eq!(entry.evidence, vec!["interpreter:bash"]);
    assert_eq!(
        entry.selector.target_fingerprint.as_deref(),
        Some("git-mode:100755")
    );
}

#[test]
fn executable_findings_read_git_stage_executable_paths() {
    let stage = "\
100644 abc 0\tREADME.md\n\
100755 def 0\tscripts/package-proof.sh\n\
120000 ghi 0\tscripts/link.sh\n";

    let findings = executable_findings_from_git_stage(stage);

    assert_eq!(findings.len(), 1);
    let finding = findings
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected executable finding"));
    assert_eq!(finding.kind, FindingKind::PolicyException);
    assert_eq!(finding.family.as_deref(), Some("executable_file"));
    assert_eq!(finding.path, PathBuf::from("scripts/package-proof.sh"));
    assert_eq!(finding.identity.ast_kind, "git_executable_file");
    assert_eq!(
        finding.identity.target_fingerprint.as_deref(),
        Some("git-mode:100755")
    );
}

#[test]
fn executable_compat_preserves_missing_and_stale_drift() {
    let policy = executable_policy_fixture_path();
    let cfg = load_executable_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("executable compat config loads: {err}"))
    });

    let matched = allow_match::evaluate(
        &cfg,
        &[executable_finding(PathBuf::from(
            "scripts/package-proof.sh",
        ))],
        allow_match::CheckMode::NoNew,
    );
    assert!(matched.iter().any(|outcome| {
        outcome.status == allow_core::MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("exec-package-proof")
    }));

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[executable_finding(PathBuf::from("scripts/new-tool.sh"))],
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
fn migrates_process_allowlist_to_policy_exception_entries() {
    let policy = process_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("process policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let install = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "proc-cargo-install-cargo-deny")
        .unwrap_or_else(|| std::panic::panic_any("expected cargo install process entry"));
    assert_eq!(install.kind, FindingKind::PolicyException);
    assert_eq!(install.family.as_deref(), Some("process_spawn"));
    assert_eq!(install.classification, "network_process");
    assert_eq!(
        install.path.as_deref(),
        Some(Path::new(".github/workflows/ci.yml"))
    );
    assert_eq!(install.selector.ast_kind.as_deref(), Some("process_spawn"));
    assert_eq!(
        install.selector.symbol.as_deref(),
        Some("cargo install cargo-deny --locked")
    );
    assert_eq!(
        install.selector.target_fingerprint.as_deref(),
        Some("process:cargo install cargo-deny --locked")
    );
    assert_eq!(
        install.lifecycle.review_after.as_deref(),
        Some("2026-09-09")
    );
    assert!(
        install
            .evidence
            .iter()
            .any(|item| item == "network_reach:true")
    );

    let local = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "proc-bash-package-proof")
        .unwrap_or_else(|| std::panic::panic_any("expected package proof process entry"));
    assert_eq!(local.classification, "local_process");
    assert_eq!(local.lifecycle.expires.as_deref(), Some("never"));
}

#[test]
fn process_compat_synthesizes_matched_new_and_stale_drift() {
    let policy = process_policy_fixture_path();
    let cfg = load_process_compat_config(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("process compat config loads: {err}")));
    let findings = process_findings_from_config(&cfg);

    let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[process_policy_finding(
            ".github/workflows/release.yml",
            "bash scripts/publish.sh",
        )],
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
fn process_policy_requires_legacy_xtask_fields() {
    let policy = malformed_process_policy_fixture_path();
    let err = load_process_compat_config(&policy)
        .expect_err("process policy without network_reach should fail");
    assert!(
        err.to_string()
            .contains("proc-missing missing network_reach")
    );
}

#[test]
fn migrates_network_allowlist_to_policy_exception_entries() {
    let policy = network_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("network policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let public = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "net-crates-io-fetch")
        .unwrap_or_else(|| std::panic::panic_any("expected crates.io network entry"));
    assert_eq!(public.kind, FindingKind::PolicyException);
    assert_eq!(public.family.as_deref(), Some("network_destination"));
    assert_eq!(public.classification, "public_network");
    assert_eq!(
        public.path.as_deref(),
        Some(Path::new("policy/network-allowlist.toml"))
    );
    assert_eq!(
        public.selector.ast_kind.as_deref(),
        Some("network_destination")
    );
    assert_eq!(
        public.selector.symbol.as_deref(),
        Some("crates.io lane build")
    );
    assert_eq!(
        public.selector.target_fingerprint.as_deref(),
        Some("network:crates.io:auth:false:lane:build")
    );
    assert_eq!(public.lifecycle.expires.as_deref(), Some("never"));

    let authenticated = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "net-github-api")
        .unwrap_or_else(|| std::panic::panic_any("expected GitHub API network entry"));
    assert_eq!(authenticated.classification, "authenticated_network");
    assert!(
        authenticated
            .evidence
            .iter()
            .any(|item| item == "auth_secret:GITHUB_TOKEN")
    );
}

#[test]
fn network_compat_synthesizes_matched_new_and_stale_drift() {
    let policy = network_policy_fixture_path();
    let cfg = load_network_compat_config(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("network compat config loads: {err}")));
    let findings = network_findings_from_config(&cfg);

    let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[network_policy_finding("example.com lane test")],
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
fn network_policy_requires_legacy_xtask_fields() {
    let policy = malformed_network_policy_fixture_path();
    let err = load_network_compat_config(&policy)
        .expect_err("network policy without auth_required should fail");
    assert!(
        err.to_string()
            .contains("net-missing missing auth_required")
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
