use super::*;
use crate::test_support::*;
use std::fs;
use std::path::Path;

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
fn policy_directory_includes_lint_and_unsafe_legacy_policies() {
    let dir = fixture_dir();
    fs::write(
        dir.join("clippy-exceptions.toml"),
        clippy_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("clippy fixture write: {err}")));
    fs::write(
        dir.join("unsafe-allowlist.toml"),
        unsafe_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));

    let cfg = load_legacy_policy_dir(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy directory migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.kind == allow_core::FindingKind::LintException
                && entry.selector.lint.as_deref() == Some("clippy::unwrap_used")),
        "repo-policy migration should include legacy lint exceptions"
    );
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.kind == allow_core::FindingKind::Unsafe
                && entry.family.as_deref() == Some("unsafe_block")
                && entry
                    .evidence
                    .iter()
                    .any(|item| item.starts_with("unsafe-review:"))),
        "repo-policy migration should preserve reviewed unsafe evidence"
    );
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.kind == allow_core::FindingKind::Unsafe
                && entry.family.as_deref() == Some("unsafe_fn")
                && entry.classification == "baseline_debt"
                && entry
                    .evidence
                    .iter()
                    .any(|item| item.contains("TODO: add unsafe-review"))),
        "repo-policy migration should keep unsafe entries without evidence visibly temporary"
    );
}

#[test]
fn policy_directory_includes_no_panic_legacy_policies() {
    let dir = fixture_dir();
    fs::write(
        dir.join("no-panic-baseline.toml"),
        no_panic_baseline_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("baseline fixture write: {err}")));
    fs::write(
        dir.join("no-panic-allowlist.toml"),
        no_panic_allowlist_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("allowlist fixture write: {err}")));

    let cfg = load_legacy_policy_dir(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy directory migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.id == "panic-baseline-0001"
                && entry.kind == allow_core::FindingKind::Panic
                && entry.family.as_deref() == Some("unwrap")
                && entry.classification == "baseline_debt"
                && entry.occurrence_limit == Some(2)
                && entry.evidence.iter().any(|item| item == "baseline_count:2")),
        "repo-policy migration should preserve no-panic baseline occurrence limits"
    );
    assert!(
        cfg.allow.iter().any(|entry| entry.id == "no-panic-unwrap"
            && entry.kind == allow_core::FindingKind::Panic
            && entry.family.as_deref() == Some("unwrap")
            && entry.classification == "reviewed_panic_exception"
            && entry.selector.ast_kind.as_deref() == Some("method_call")
            && entry.selector.callee.as_deref() == Some("unwrap")
            && entry.selector.container.as_deref() == Some("load")),
        "repo-policy migration should preserve reviewed no-panic structural selectors"
    );
}

#[test]
fn policy_directory_includes_file_workflow_and_dependency_policies() {
    let dir = fixture_dir();
    fs::write(
        dir.join("generated-allowlist.toml"),
        generated_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("generated fixture write: {err}")));
    fs::write(
        dir.join("executable-allowlist.toml"),
        executable_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("executable fixture write: {err}")));
    fs::write(
        dir.join("workflow-allowlist.toml"),
        workflow_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("workflow fixture write: {err}")));
    fs::write(
        dir.join("dependency-surface-allowlist.toml"),
        dependency_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("dependency fixture write: {err}")));

    let cfg = load_legacy_policy_dir(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy directory migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 7);
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.kind == allow_core::FindingKind::GeneratedCode
                && entry.family.as_deref() == Some("generated_code")
                && entry.path.as_deref() == Some(Path::new("policy/no-panic-baseline.toml"))),
        "repo-policy migration should include generated-code legacy policy entries"
    );
    assert!(
        cfg.allow.iter().any(
            |entry| entry.kind == allow_core::FindingKind::PolicyException
                && entry.family.as_deref() == Some("executable_file")
                && entry.path.as_deref() == Some(Path::new("scripts/package-proof.sh"))
                && entry.selector.target_fingerprint.as_deref() == Some("git-mode:100755")
        ),
        "repo-policy migration should include executable-file legacy policy entries"
    );
    assert!(
        cfg.allow.iter().any(
            |entry| entry.kind == allow_core::FindingKind::PolicyException
                && entry.family.as_deref() == Some("github_workflow")
                && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml"))
        ),
        "repo-policy migration should include workflow-file legacy policy entries"
    );
    assert!(
        cfg.allow.iter().any(
            |entry| entry.kind == allow_core::FindingKind::PolicyException
                && entry.family.as_deref() == Some("workflow_external_action")
                && entry.selector.target_fingerprint.as_deref()
                    == Some("action:actions/checkout@v6.0.2")
        ),
        "repo-policy migration should include workflow external-action entries"
    );
    assert!(
        cfg.allow.iter().any(
            |entry| entry.kind == allow_core::FindingKind::PolicyException
                && entry.family.as_deref() == Some("dependency_surface")
                && entry.path.as_deref() == Some(Path::new("Cargo.toml"))
                && entry
                    .evidence
                    .iter()
                    .any(|item| item == "dep_count_at_baseline:22")
        ),
        "repo-policy migration should include dependency-surface legacy policy entries"
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
