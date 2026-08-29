use super::*;
use crate::test_support::*;
use allow_core::FindingKind;
use std::fs;
use std::path::Path;

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
    assert_eq!(
        entry.evidence,
        vec!["legacy-policy:clippy-unwrap-policy".to_string()]
    );
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        0,
        "clippy migration should not create synthetic weak lint: evidence"
    );
    assert_eq!(
        entry.lifecycle.review_after.as_deref(),
        Some(crate::test_fixture_text::live_review_after().as_str())
    );
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
    assert_eq!(
        entry.evidence,
        vec!["legacy-policy:legacy-clippy-0000".to_string()]
    );
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        0,
        "minimal clippy migration should preserve recognized legacy evidence"
    );
    assert_current_baseline_window(&entry.lifecycle);
}

#[test]
fn clippy_compat_preserves_legacy_evidence_when_present() {
    let path = fixture_dir().join("clippy-exceptions-with-evidence.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "clippy-exceptions"

[[allow]]
id = "clippy-reviewed"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
evidence = ["test:lint_policy_is_linked", "issue:#123"]
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_clippy_exceptions_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("clippy compat config with evidence loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
    assert_eq!(
        entry.evidence,
        vec![
            "test:lint_policy_is_linked".to_string(),
            "issue:#123".to_string()
        ]
    );
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        0,
        "recognized legacy clippy evidence should not be reported as weak"
    );
}

#[test]
fn clippy_compat_accepts_covered_by_as_legacy_evidence() {
    let path = fixture_dir().join("clippy-exceptions-covered-by.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "clippy-exceptions"

[[allow]]
id = "clippy-covered"
path = "src/lib.rs"
lint = "clippy::panic"
covered_by = "test:lint_policy_covered"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_clippy_exceptions_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("clippy compat config with covered_by loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
    assert_eq!(entry.evidence, vec!["test:lint_policy_covered".to_string()]);
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
            .any(|item| item == &format!("legacy-policy:{}", generated.id))
    );
    assert!(
        generated
            .evidence
            .iter()
            .any(|item| item.contains("TODO: add unsafe-review"))
    );
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        1,
        "generated unsafe fallback should keep the TODO weak-evidence route"
    );
    assert_current_baseline_window(&generated.lifecycle);
}

#[test]
fn unsafe_allowlist_compat_preserves_matched_new_and_stale_drift() {
    let policy = unsafe_policy_fixture_path();
    let cfg = load_unsafe_allowlist_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("unsafe allowlist compat config loads: {err}"))
    });

    let mut finding = unsafe_finding("src/lib.rs", "unsafe_block", Some("read"));
    finding.span = Some(allow_core::Span {
        line: 7,
        column: 12,
    });
    let matched = allow_match::evaluate(&cfg, &[finding], allow_match::CheckMode::NoNew);
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
