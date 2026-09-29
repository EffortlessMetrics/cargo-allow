use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;
use serde_json::Value;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_cadence_args() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "cadence",
        "--as-of",
        "2026-10-01",
        "--format",
        "json",
        "--output",
        "target/cadence.json",
        "--config",
        "policy/allow.toml",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse cadence args: {err}")));

    let Some(CargoAllowCommand::Cadence(args)) = parsed.command else {
        std::panic::panic_any("cadence subcommand should parse into Cadence args");
    };
    assert_eq!(
        args.as_of.map(|date| date.to_string()),
        Some("2026-10-01".to_string())
    );
    assert!(matches!(args.format, CadenceFormat::Json));
    assert_eq!(
        args.output.as_deref(),
        Some(std::path::Path::new("target/cadence.json"))
    );
    assert_eq!(
        args.config.as_deref(),
        Some(std::path::Path::new("policy/allow.toml"))
    );
}

#[test]
fn clap_parses_cadence_markdown_and_md_aliases() {
    for value in ["markdown", "md"] {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "cadence", "--format", value]))
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("CLI should parse --format {value}: {err}"))
                });
        let Some(CargoAllowCommand::Cadence(args)) = parsed.command else {
            std::panic::panic_any("cadence subcommand should parse into Cadence args");
        };
        assert!(
            matches!(args.format, CadenceFormat::Markdown),
            "--format {value} should select the markdown rendering"
        );
    }
}

#[test]
fn clap_defaults_cadence_to_human_format_and_ambient_as_of() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "cadence"]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse bare cadence: {err}"))
        });
    let Some(CargoAllowCommand::Cadence(args)) = parsed.command else {
        std::panic::panic_any("cadence subcommand should parse into Cadence args");
    };
    assert!(matches!(args.format, CadenceFormat::Human));
    assert!(
        args.as_of.is_none(),
        "missing --as-of defers to the ambient day"
    );
    assert!(args.output.is_none());
}

#[test]
fn clap_rejects_malformed_as_of_loudly() {
    for bad in [
        "not-a-date",
        "2026-13-40",
        "20261001",
        "2026-10-01T00:00:00Z",
    ] {
        let err = match CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "cadence",
            "--as-of",
            bad,
        ])) {
            Err(err) => err,
            Ok(_) => {
                std::panic::panic_any("malformed --as-of must be rejected at argument parsing")
            }
        };
        let message = err.to_string();
        assert!(
            message.contains("invalid --as-of date") && message.contains(bad),
            "malformed --as-of `{bad}` should fail with the cadence date message: {message}"
        );
    }
}

#[test]
fn parse_as_of_arg_accepts_valid_dates_and_names_the_field() {
    let parsed = parse_as_of_arg("2026-10-01")
        .unwrap_or_else(|err| std::panic::panic_any(format!("valid date should parse: {err}")));
    assert_eq!(parsed.to_string(), "2026-10-01");
    let err = match parse_as_of_arg("2026-02-30") {
        Err(err) => err,
        Ok(_) => std::panic::panic_any("impossible calendar date must be rejected"),
    };
    assert!(
        err.contains("--as-of") && err.contains("2026-02-30"),
        "error should name the argument and value: {err}"
    );
}

#[test]
fn sample_cadence_json_covers_six_classes_and_carries_no_verdict_fields() {
    let json = sample_cadence_json_for_contract_test();
    let value: Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("sample cadence JSON parses: {err}\n{json}"))
    });
    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some("cargo-allow.cadence.v1")
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("cadence")
    );
    assert_eq!(
        value.pointer("/as_of").and_then(Value::as_str),
        Some("2026-10-01")
    );
    assert_eq!(
        value.pointer("/as_of_source").and_then(Value::as_str),
        Some("explicit")
    );

    let rows = value
        .pointer("/rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("cadence sample should carry rows"));
    assert_eq!(
        rows.len(),
        6,
        "sample should carry one row per lifecycle class"
    );
    let mut classes: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.pointer("/class").and_then(Value::as_str))
        .collect();
    classes.sort_unstable();
    classes.dedup();
    assert_eq!(
        classes.len(),
        6,
        "all six classes must be distinct: {classes:?}"
    );

    // Never a gate: the cadence artifact carries no check/diff verdict
    // vocabulary at the document or row level.
    for verdict_key in [
        "failed",
        "passed",
        "status",
        "mode",
        "verdict",
        "advisory",
        "enforcement",
    ] {
        assert!(
            value.get(verdict_key).is_none(),
            "cadence artifact must not carry verdict field {verdict_key}"
        );
    }
    for row in rows {
        for verdict_key in ["failed", "passed", "status", "mode", "verdict"] {
            assert!(
                row.get(verdict_key).is_none(),
                "cadence row must not carry verdict field {verdict_key}"
            );
        }
        assert!(
            row.get("required_disposition")
                .and_then(Value::as_str)
                .is_some_and(|d| !d.is_empty()),
            "every cadence row carries its required disposition"
        );
    }
}

// --- Classifier tests (#4239): the class decision reuses the match-engine
// and loader lifecycle predicates verbatim; these tests pin the cadence
// precedence law and its boundaries.

use allow_core::{FindingKind, Lifecycle, Selector};

const CLASSIFIER_AS_OF: &str = "2026-10-01";

fn classifier_as_of() -> SimpleDate {
    SimpleDate::parse(CLASSIFIER_AS_OF)
        .unwrap_or_else(|| std::panic::panic_any("fixed classifier as-of must parse"))
}

/// Date `offset` days from the fixed classifier as-of date.
fn classifier_day(offset: i64) -> String {
    classifier_as_of().add_days(offset).to_string()
}

fn classifier_entry(id: &str, review_after: Option<&str>, expires: Option<&str>) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(std::path::PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core/parser".to_string(),
        classification: "reviewed".to_string(),
        reason: "cadence classifier fixture".to_string(),
        evidence: vec!["test:cadence_classifier".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-06-01".to_string()),
            review_after: review_after.map(str::to_string),
            expires: expires.map(str::to_string),
        },
        selector: Selector {
            ast_kind: Some("call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn sole_row(entries: &[AllowEntry]) -> allow_report::LifecycleCadenceRowV1 {
    let mut rows = lifecycle_cadence_rows(entries, classifier_as_of());
    let Some(row) = rows.pop() else {
        std::panic::panic_any("the single-entry classifier fixture must produce one row");
    };
    row
}

fn class_of(review_after: Option<&str>, expires: Option<&str>) -> LifecycleCadenceClassV1 {
    sole_row(&[classifier_entry("allow-x", review_after, expires)]).class
}

#[test]
fn review_boundary_is_inclusive_overdue_and_horizon_bounded() {
    // review_after == as_of is overdue (match-engine `<=`, #2008).
    assert_eq!(
        class_of(Some(CLASSIFIER_AS_OF), None),
        LifecycleCadenceClassV1::ReviewOverdue
    );
    // One day past the deadline stays overdue.
    assert_eq!(
        class_of(Some(&classifier_day(-1)), None),
        LifecycleCadenceClassV1::ReviewOverdue
    );
    // Last day inside the due-soon horizon.
    assert_eq!(
        class_of(Some(&classifier_day(14)), None),
        LifecycleCadenceClassV1::ReviewDueSoon
    );
    // One day beyond the horizon is current.
    assert_eq!(
        class_of(Some(&classifier_day(15)), None),
        LifecycleCadenceClassV1::Current
    );
}

#[test]
fn expiry_boundary_is_strict_and_expiring_on_the_day() {
    // The expires day itself is NOT expired (match-engine `<`, #2008);
    // it is expiring with zero days remaining.
    let on_the_day = sole_row(&[classifier_entry("allow-e", None, Some(CLASSIFIER_AS_OF))]);
    assert_eq!(on_the_day.class, LifecycleCadenceClassV1::Expiring);
    assert_eq!(on_the_day.days_remaining, Some(0));
    // Last day inside the expiring horizon.
    assert_eq!(
        class_of(None, Some(&classifier_day(14))),
        LifecycleCadenceClassV1::Expiring
    );
    // One day beyond the horizon is current.
    assert_eq!(
        class_of(None, Some(&classifier_day(15))),
        LifecycleCadenceClassV1::Current
    );
    // Strictly past expires is expired with a negative delta.
    let expired = sole_row(&[classifier_entry("allow-e", None, Some(&classifier_day(-1)))]);
    assert_eq!(expired.class, LifecycleCadenceClassV1::Expired);
    assert_eq!(expired.days_remaining, Some(-1));
}

#[test]
fn precedence_follows_the_module_law() {
    // expired beats expiring beats review_overdue.
    assert_eq!(
        class_of(Some(&classifier_day(-30)), Some(&classifier_day(-1))),
        LifecycleCadenceClassV1::Expired
    );
    assert_eq!(
        class_of(Some(&classifier_day(-30)), Some(&classifier_day(3))),
        LifecycleCadenceClassV1::Expiring
    );
    // "never" never expires; an overdue review still surfaces.
    assert_eq!(
        class_of(Some(&classifier_day(-2)), Some("never")),
        LifecycleCadenceClassV1::ReviewOverdue
    );
    // No dates at all is current.
    assert_eq!(class_of(None, None), LifecycleCadenceClassV1::Current);
}

#[test]
fn malformed_and_descending_dates_classify_invalid_fail_closed() {
    // The policy loader rejects every one of these shapes at parse
    // (`allow_policy::lifecycle::validate_lifecycle`, reused verbatim as
    // the `invalid` law), so a CLI cadence run fails loudly instead; these
    // cases pin the typed classifier's defense-in-depth behavior for
    // in-memory entries.
    let fail_closed = LifecycleCadenceClassV1::Invalid.required_disposition();
    assert_eq!(fail_closed, "fix malformed lifecycle date (fail-closed)");
    let bad_expires = classifier_entry("allow-bad-expires", None, Some("2026-13-40"));
    let bad_review = classifier_entry("allow-bad-review", Some("not-a-date"), None);
    let descending_review = {
        let mut entry = classifier_entry("allow-descending-review", Some("2026-01-01"), None);
        entry.lifecycle.created = Some("2026-06-01".to_string());
        entry
    };
    let descending_expires = {
        let mut entry = classifier_entry("allow-descending-expires", None, Some("2026-01-01"));
        entry.lifecycle.created = Some("2026-06-01".to_string());
        entry
    };
    let review_past_expiry = classifier_entry(
        "allow-review-past-expiry",
        Some("2026-12-01"),
        Some("2026-11-01"),
    );
    for case in [
        bad_expires,
        bad_review,
        descending_review,
        descending_expires,
        review_past_expiry,
    ] {
        let row = sole_row(&[case]);
        assert_eq!(
            row.class,
            LifecycleCadenceClassV1::Invalid,
            "{}",
            row.allow_id
        );
        assert_eq!(row.required_disposition, fail_closed, "{}", row.allow_id);
        assert_eq!(row.days_remaining, None, "{}", row.allow_id);
        assert_eq!(row.driving_date.as_str(), "none", "{}", row.allow_id);
    }
}

#[test]
fn six_class_totality_is_exactly_one_class_per_entry() {
    let entries = vec![
        classifier_entry("allow-current", Some("2027-06-01"), None),
        classifier_entry("allow-due-soon", Some(&classifier_day(10)), None),
        classifier_entry("allow-overdue", Some(&classifier_day(-2)), None),
        classifier_entry("allow-expiring", None, Some(&classifier_day(5))),
        classifier_entry("allow-expired", None, Some(&classifier_day(-5))),
        classifier_entry("allow-invalid", Some("oops"), None),
    ];
    let rows = lifecycle_cadence_rows(&entries, classifier_as_of());
    let mut classes: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.allow_id.clone(), row.class.as_str().to_string()))
        .collect();
    classes.sort();
    let expected = vec![
        ("allow-current".to_string(), "current".to_string()),
        ("allow-due-soon".to_string(), "review_due_soon".to_string()),
        ("allow-expired".to_string(), "expired".to_string()),
        ("allow-expiring".to_string(), "expiring".to_string()),
        ("allow-invalid".to_string(), "invalid".to_string()),
        ("allow-overdue".to_string(), "review_overdue".to_string()),
    ];
    assert_eq!(
        classes, expected,
        "six-class totality: each fixture entry must land in exactly its class"
    );
    // Deterministic order: class rank, then allow id.
    for (left, right) in rows.iter().zip(rows.iter().skip(1)) {
        let ordered = left.class.rank() < right.class.rank()
            || (left.class.rank() == right.class.rank() && left.allow_id <= right.allow_id);
        assert!(ordered, "rows must be sorted by (class rank, allow id)");
    }
}

#[test]
fn same_policy_and_as_of_classify_identically_twice() {
    let entries = vec![
        classifier_entry("allow-due-soon", Some(&classifier_day(4)), None),
        classifier_entry("allow-expired", None, Some(&classifier_day(-3))),
        classifier_entry("allow-current", Some("2027-06-01"), None),
    ];
    let first = lifecycle_cadence_rows(&entries, classifier_as_of());
    let second = lifecycle_cadence_rows(&entries, classifier_as_of());
    assert_eq!(first, second, "same (policy, as_of) classifies identically");
}
