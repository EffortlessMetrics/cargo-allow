use super::*;
use crate::test_support::*;
use std::path::Path;

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
    assert_eq!(docs.lifecycle.review_after.as_deref(), Some("2026-05-09"));
    assert!(docs.reason.contains("Scope note:"));
    assert_eq!(
        docs.evidence,
        vec![
            "legacy-policy:non-rust-docs",
            "TODO: add non-rust migration evidence",
        ]
    );
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
    assert_eq!(
        entry.evidence,
        vec![
            "legacy-policy:non-rust-github-workflows",
            "TODO: add non-rust migration evidence",
        ]
    );
    assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
}

#[test]
fn non_rust_migration_preserves_legacy_evidence_when_present() {
    let policy = non_rust_policy_with_entry(
        r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "Docs are maintained as source-tree governance surfaces."
evidence = ["doc:docs/source-exception-ledger.md", "issue:#123"]
created = "2026-05-09"
expires = "permanent"
"#,
    );

    let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("legacy non-rust policy migrates: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected non-rust allow entry"));
    assert_eq!(
        entry.evidence,
        vec![
            "doc:docs/source-exception-ledger.md".to_string(),
            "issue:#123".to_string()
        ]
    );
}

#[test]
fn non_rust_compat_preserves_covered_by_as_legacy_evidence() {
    let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];
    let policy = non_rust_policy_with_entry(
        r#"id = "non-rust-workflows"
glob = ".github/workflows/*.yml"
category = "ci_declarative"
owner = "release/ci"
reason = "Workflows are reviewed by release engineering."
broad_glob_reason = "GitHub workflow files are all declarative CI policy."
covered_by = "doc:docs/ci.md"
created = "2026-05-09"
review_after = "2026-11-01"
"#,
    );

    let cfg = load_non_rust_compat_config(&policy, &findings)
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy compat config loads: {err}")));

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one compat allow entry"));
    assert_eq!(entry.evidence, vec!["doc:docs/ci.md"]);
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
