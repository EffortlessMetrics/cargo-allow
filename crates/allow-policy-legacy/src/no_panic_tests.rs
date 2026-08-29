use super::*;
use crate::test_support::*;
use allow_core::FindingKind;
use std::{fs, path::Path};

#[test]
fn migrates_no_panic_baseline_to_count_limited_baseline_debt() {
    let policy = no_panic_baseline_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic baseline migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let unwrap = cfg
        .allow
        .iter()
        .find(|entry| entry.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap baseline entry"));
    assert_eq!(unwrap.kind, FindingKind::Panic);
    assert_eq!(unwrap.classification, "baseline_debt");
    assert_eq!(unwrap.owner, "unowned");
    assert_eq!(unwrap.occurrence_limit, Some(2));
    assert_current_baseline_window(&unwrap.lifecycle);
    assert_eq!(unwrap.selector.ast_kind.as_deref(), Some("method_call"));
    assert_eq!(unwrap.selector.callee.as_deref(), Some("unwrap"));
    assert!(unwrap.selector.normalized_snippet_hash.is_some());
    assert!(
        unwrap
            .evidence
            .iter()
            .any(|item| item == "baseline_count:2")
    );

    let panic = cfg
        .allow
        .iter()
        .find(|entry| entry.family.as_deref() == Some("panic_macro"))
        .unwrap_or_else(|| std::panic::panic_any("expected panic macro baseline entry"));
    assert_eq!(panic.selector.ast_kind.as_deref(), Some("macro_call"));
    assert_eq!(panic.selector.macro_name.as_deref(), Some("panic"));
    assert_eq!(panic.occurrence_limit, Some(1));
}

#[test]
fn no_panic_compat_loader_requires_no_panic_policy() {
    let policy = generated_policy_fixture_path();

    let err = load_no_panic_baseline_compat_config(&policy)
        .expect_err("generated policy should not load as no-panic compat");

    assert!(err.to_string().contains("not a no-panic-baseline policy"));
}

#[test]
fn no_panic_baseline_occurrence_limit_prevents_unbounded_matches() {
    let policy = no_panic_baseline_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic baseline migrates: {err}")));
    let snippet = ["let value = maybe.", "unwrap();"].concat();
    let finding = panic_finding(
        "src/lib.rs",
        "unwrap",
        "method_call",
        Some("unwrap"),
        None,
        &snippet,
    );

    let outcomes = allow_match::evaluate(
        &cfg,
        &[finding.clone(), finding.clone(), finding],
        allow_match::CheckMode::NoNew,
    );

    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::New
                && outcome.message.contains("occurrence_limit exceeded"))
    );
}

#[test]
fn migrates_no_panic_allowlist_to_structural_panic_entries() {
    let policy = no_panic_allowlist_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic allowlist migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let unwrap = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "no-panic-unwrap")
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap allow entry"));
    assert_eq!(unwrap.kind, FindingKind::Panic);
    assert_eq!(unwrap.family.as_deref(), Some("unwrap"));
    assert_eq!(unwrap.reason, "Parser validates the optional value.");
    assert_eq!(unwrap.selector.ast_kind.as_deref(), Some("method_call"));
    assert_eq!(unwrap.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(unwrap.selector.container.as_deref(), Some("load"));
    assert_eq!(unwrap.selector.line_hint, Some(7));
    assert_eq!(
        unwrap
            .last_seen
            .as_ref()
            .map(|seen| (seen.line, seen.column)),
        Some((7, 12))
    );
    assert_eq!(
        unwrap.lifecycle.review_after.as_deref(),
        Some(crate::test_fixture_text::live_review_after().as_str())
    );

    let generated = cfg
        .allow
        .iter()
        .find(|entry| entry.id.starts_with("legacy-no-panic-"))
        .unwrap_or_else(|| std::panic::panic_any("expected generated no-panic entry"));
    assert_eq!(generated.classification, "baseline_debt");
    assert_eq!(generated.owner, "unowned");
    assert_eq!(generated.selector.macro_name.as_deref(), Some("panic"));
    assert_current_baseline_window(&generated.lifecycle);
}

#[test]
fn no_panic_allowlist_preserves_legacy_evidence_when_present() {
    let path = fixture_dir().join("no-panic-allowlist-with-evidence.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "no-panic-allowlist"

[[allow]]
id = "no-panic-reviewed"
path = "src/lib.rs"
family = "unwrap"
reason = "Parser validates optional value."
evidence = ["test:parser_validates_optional_value", "issue:#123"]

[allow.selector]
kind = "method-call"
callee = "unwrap"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_no_panic_allowlist_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("no-panic allowlist with evidence loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected no-panic allow entry"));
    assert_eq!(
        entry.evidence,
        vec![
            "test:parser_validates_optional_value".to_string(),
            "issue:#123".to_string()
        ]
    );
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        0,
        "recognized legacy no-panic evidence should not be reported as weak"
    );
}

#[test]
fn no_panic_allowlist_accepts_covered_by_as_legacy_evidence() {
    let path = fixture_dir().join("no-panic-allowlist-covered-by.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "no-panic-allowlist"

[[allow]]
id = "no-panic-covered"
path = "src/lib.rs"
family = "panic"
explanation = "Crash path is unreachable after argument validation."
covered_by = "test:panic_path_unreachable"

[allow.selector]
kind = "macro-call"
callee = "panic"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_no_panic_allowlist_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("no-panic allowlist with covered_by loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected no-panic allow entry"));
    assert_eq!(
        entry.evidence,
        vec!["test:panic_path_unreachable".to_string()]
    );
}

#[test]
fn no_panic_allowlist_compat_preserves_matched_new_and_stale_drift() {
    let policy = no_panic_allowlist_fixture_path();
    let cfg = load_no_panic_allowlist_compat_config(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("no-panic allowlist compat config loads: {err}"))
    });

    let mut finding = panic_finding(
        "src/lib.rs",
        "unwrap",
        "method_call",
        Some("unwrap"),
        None,
        "let value = maybe.unwrap();",
    );
    finding.identity.container = Some("load".to_string());
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
        &[panic_finding(
            "src/lib.rs",
            "expect",
            "method_call",
            Some("expect"),
            None,
            "let value = maybe.expect(\"value\");",
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
fn no_panic_allowlist_loader_requires_allowlist_policy() {
    let policy = no_panic_baseline_fixture_path();

    let err = load_no_panic_allowlist_compat_config(&policy)
        .expect_err("baseline policy should not load as no-panic allowlist compat");

    assert!(err.to_string().contains("not a no-panic-allowlist policy"));
}

#[test]
fn no_panic_baseline_preserves_legacy_evidence_when_present() {
    let path = fixture_dir().join("no-panic-baseline-with-evidence.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "no-panic-baseline"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "let value = maybe.unwrap();"
count = 2
owner = "parser"
reason = "Counted unwrap baseline after parser hardening."
evidence = ["test:parser_validates_optional_value", "issue:#123"]
created = "2026-05-09"
review_after = "2026-06-09"
expires = "2026-06-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_no_panic_baseline_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("no-panic baseline with evidence loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected no-panic baseline entry"));
    assert_eq!(entry.owner, "parser");
    assert_eq!(
        entry.reason,
        "Counted unwrap baseline after parser hardening."
    );
    assert_eq!(entry.occurrence_limit, Some(2));
    assert_eq!(
        entry.evidence,
        vec![
            "test:parser_validates_optional_value".to_string(),
            "issue:#123".to_string(),
        ]
    );
    assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-09"));
    assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-06-09"));
    assert_eq!(entry.lifecycle.expires.as_deref(), Some("2026-06-09"));
    assert_eq!(
        allow_policy::weak_evidence_reference_count(Path::new("."), &cfg),
        0,
        "recognized legacy no-panic baseline evidence should not be reported as weak"
    );
}

#[test]
fn no_panic_baseline_without_evidence_keeps_visible_debt_markers() {
    let path = fixture_dir().join("no-panic-baseline-debt.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "no-panic-baseline"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "let value = maybe.unwrap();"
count = 2
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

    let cfg = load_no_panic_baseline_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("no-panic baseline debt fixture loads: {err}"))
    });

    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected no-panic baseline entry"));
    assert_eq!(entry.owner, "unowned");
    assert_eq!(entry.classification, "baseline_debt");
    assert_eq!(
        entry.evidence,
        vec![
            "legacy_policy:no-panic-baseline".to_string(),
            "legacy_selector_callee:Option/Result::unwrap".to_string(),
            "baseline_count:2".to_string(),
        ]
    );
}
