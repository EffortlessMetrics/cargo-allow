use super::test_support::test_finding_at_line;
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;

#[test]
fn clap_parses_add_from_finding() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "add",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "42",
        "--owner",
        "parser",
        "--reason",
        "validated invariant",
        "--evidence",
        "test:parser_invariant",
        "--write",
        "policy/allow.proposed.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/add-summary.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Add(AddArgs {
            kind,
            path,
            line: 42,
            owner,
            reason,
            evidence,
            write: Some(write),
            force: true,
            summary_format: AddSummaryFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if kind == "panic"
            && path == Path::new("src/lib.rs")
            && owner == "parser"
            && reason == "validated invariant"
            && evidence == vec!["test:parser_invariant".to_string()]
            && write == Path::new("policy/allow.proposed.toml")
            && summary_output == Path::new("target/add-summary.json")
    ));
}

#[test]
fn select_add_finding_picks_nearest_path_and_kind() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            10,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Unsafe,
            Some("unsafe_fn"),
            "src/lib.rs",
            "unsafe_fn",
            39,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let (_index, selected) = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 39)
        .unwrap_or_else(|err| std::panic::panic_any(format!("finding should select: {err}")));

    assert_eq!(selected.family.as_deref(), Some("expect"));
    assert_eq!(selected.span.as_ref().map(|span| span.line), Some(40));
}

#[test]
fn select_add_finding_fails_closed_on_equal_nearest_findings() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            42,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let err = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 41)
        .expect_err("equally near findings should be ambiguous");

    assert!(err.to_string().contains("ambiguous add request"));
}

#[test]
fn ensure_addable_outcome_rejects_already_matched_findings() {
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

    let err = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched finding should not be addable");

    assert!(err.to_string().contains("already receipted"));
}

#[test]
fn allow_entry_from_finding_uses_structural_selector_and_review_metadata() {
    let mut finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    finding.identity.container = Some("parse_span".to_string());
    finding.identity.callee = Some("unwrap".to_string());
    finding.identity.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

    let entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0099".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });

    assert_eq!(entry.id, "allow-0099");
    assert_eq!(entry.owner, "parser");
    assert_eq!(entry.selector.container.as_deref(), Some("parse_span"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(
        entry.selector.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:1234")
    );
    assert_eq!(entry.last_seen.as_ref().map(|last| last.line), Some(42));
}

#[test]
fn default_add_review_after_is_relative_to_current_date() {
    let before = allow_core::SimpleDate::today_utc_approx().add_days(ADD_REVIEW_AFTER_DEFAULT_DAYS);
    let review_after = default_add_review_after();
    let after = allow_core::SimpleDate::today_utc_approx().add_days(ADD_REVIEW_AFTER_DEFAULT_DAYS);
    let parsed = allow_core::SimpleDate::parse(&review_after)
        .unwrap_or_else(|| std::panic::panic_any("default review_after should be a valid date"));

    assert!(
        before <= parsed && parsed <= after,
        "default add review_after should stay relative to the current UTC date"
    );
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
