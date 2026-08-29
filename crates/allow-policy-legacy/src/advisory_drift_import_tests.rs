use super::*;
use crate::test_support::*;
use allow_core::Span;
use std::fs;
use std::path::PathBuf;

#[test]
fn advisory_drift_import_preserves_last_seen_and_line_hint_for_clippy() {
    let policy_path = stage_advisory_drift_fixture();
    let cfg = load_clippy_exceptions_compat_config(&policy_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("clippy advisory drift fixture migration: {err}"))
    });

    let entry = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "fixture-clippy-drift")
        .unwrap_or_else(|| std::panic::panic_any("expected fixture-clippy-drift entry"));

    assert_eq!(entry.selector.line_hint, Some(14));
    assert_eq!(
        entry
            .last_seen
            .as_ref()
            .map(|last_seen| (last_seen.line, last_seen.column)),
        Some((14, 8))
    );
}

#[test]
fn advisory_drift_import_reports_location_drift_without_failing_no_new() {
    let policy_path = stage_advisory_drift_fixture();
    let cfg = load_clippy_exceptions_compat_config(&policy_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("clippy advisory drift compat config: {err}"))
    });

    let mut finding = lint_finding(
        "src/lib.rs",
        "expect_attribute",
        "clippy::unwrap_used",
        Some("fixture-clippy-drift"),
    );
    finding.span = Some(Span {
        line: 22,
        column: 4,
    });

    let outcomes = allow_match::evaluate(&cfg, &[finding], allow_match::CheckMode::NoNew);

    let drift = outcomes
        .iter()
        .find(|outcome| outcome.status == allow_core::MatchStatus::LocationDrift)
        .unwrap_or_else(|| std::panic::panic_any("expected location_drift outcome"));
    assert!(drift.message.contains("last_seen changed from 14:8"));
    assert!(
        !allow_match::CheckMode::NoNew.fails(drift.status),
        "location drift should remain advisory in no-new mode"
    );
}

fn migration_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
}

fn stage_advisory_drift_fixture() -> PathBuf {
    let dir = crate::test_support::fixture_dir();
    let source = migration_fixture_root().join("clippy-exceptions-advisory-drift.toml");
    let text = fs::read_to_string(&source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read advisory drift fixture: {err}")));
    // The strict LocationDrift assertion requires the entry to still be
    // Matched: a reached review_after demotes it to ReviewDue before drift
    // classification, so the staged copy maps the fixture's review date to
    // one relative to today instead of a hardcoded calendar date the suite
    // would eventually sail past.
    let review_after = allow_core::SimpleDate::today_utc_approx()
        .add_days(30)
        .to_string();
    let text = text.replace(
        "review_after = \"2026-09-09\"",
        &format!("review_after = \"{review_after}\""),
    );
    let path = dir.join("clippy-exceptions-advisory-drift.toml");
    fs::write(&path, text).unwrap_or_else(|err| {
        std::panic::panic_any(format!("stage advisory drift fixture: {err}"))
    });
    path
}
