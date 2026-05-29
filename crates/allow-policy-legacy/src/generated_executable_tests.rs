use super::*;
use crate::findings::{executable_finding, executable_findings_from_git_stage, generated_finding};
use crate::test_support::*;
use allow_core::FindingKind;
use std::path::{Path, PathBuf};

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
    assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-05-10"));
    assert!(entry.evidence.iter().any(|item| item.starts_with("cargo:")));
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
    assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-05-09"));
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
