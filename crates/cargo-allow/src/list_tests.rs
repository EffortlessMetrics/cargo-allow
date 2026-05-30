use super::test_support::{row_status, test_entry, test_finding, test_outcome};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;
use serde_json::Value;
use std::path::Path;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_list_json_filters() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--kind",
        "unsafe",
        "--family",
        "unsafe_fn",
        "--owner",
        "runtime",
        "--classification",
        "baseline_debt",
        "--path",
        "crates/allow-core",
        "--source-package",
        "allow-core",
        "--allow-id",
        "allow-runtime",
        "--status",
        "baseline_debt",
        "--expired",
        "--review-due",
        "--stale",
        "--baseline-debt",
        "--broad-scope",
        "--missing-evidence",
        "--format",
        "json",
        "--output",
        "target/list.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse list args: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            kind: Some(kind),
            family: Some(family),
            owner: Some(owner),
            classification: Some(classification),
            path: Some(path_filter),
            source_package: Some(source_package),
            allow_id: Some(allow_id),
            status: Some(status),
            expired: true,
            review_due: true,
            stale: true,
            baseline_debt: true,
            broad_scope: true,
            missing_evidence: true,
            format: ListFormat::Json,
            output: Some(path),
            ..
        })) if kind == "unsafe"
            && family == "unsafe_fn"
            && owner == "runtime"
            && classification == "baseline_debt"
            && path_filter == "crates/allow-core"
            && source_package == "allow-core"
            && allow_id == "allow-runtime"
            && status == "baseline_debt"
            && path == Path::new("target/list.json")
    ));
}

#[test]
fn list_rows_report_lifecycle_stale_and_baseline_status() {
    let mut cfg = AllowConfig::empty();
    let mut expired = test_entry("allow-expired", FindingKind::Panic);
    expired.lifecycle.expires = Some("2000-01-01".to_string());
    let mut review_due = test_entry("allow-review", FindingKind::Panic);
    review_due.lifecycle.review_after = Some("2000-01-01".to_string());
    let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
    baseline.classification = "baseline_debt".to_string();
    let stale = test_entry("allow-stale", FindingKind::Panic);
    cfg.allow = vec![expired, review_due, baseline, stale];
    let outcomes = vec![
        test_outcome(
            MatchStatus::Matched,
            Some("allow-expired"),
            Some(0),
            "matched",
        ),
        test_outcome(
            MatchStatus::Matched,
            Some("allow-review"),
            Some(1),
            "matched",
        ),
        test_outcome(
            MatchStatus::Matched,
            Some("allow-baseline"),
            Some(2),
            "matched",
        ),
        test_outcome(MatchStatus::Stale, Some("allow-stale"), None, "stale"),
    ];
    let expired_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-expired.file",
        "tracked_file",
    );
    let mut review_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-review.file",
        "tracked_file",
    );
    review_finding.identity.crate_name = Some("review-package".to_string());
    let stale_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-stale.file",
        "tracked_file",
    );
    let findings = vec![expired_finding, review_finding, stale_finding];

    let rows = list_rows(&cfg, &findings, &outcomes);

    assert_eq!(row_status(&rows, "allow-expired"), MatchStatus::Expired);
    assert_eq!(row_status(&rows, "allow-review"), MatchStatus::ReviewDue);
    assert_eq!(
        rows.iter()
            .find(|row| row.id == "allow-review")
            .and_then(|row| row.source_package.as_deref()),
        Some("review-package")
    );
    assert_eq!(
        row_status(&rows, "allow-baseline"),
        MatchStatus::BaselineDebt
    );
    assert_eq!(row_status(&rows, "allow-stale"), MatchStatus::Stale);
}

#[test]
fn list_rows_report_broad_scope_from_selector_glob() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-broad", FindingKind::NonRustFile);
    entry.selector.glob = Some("scripts/**".to_string());
    cfg.allow.push(entry);
    let findings = vec![test_finding(
        FindingKind::NonRustFile,
        None,
        "scripts/release.sh",
        "tracked_file",
    )];
    let outcomes = vec![test_outcome(
        MatchStatus::Matched,
        Some("allow-broad"),
        Some(0),
        "matched",
    )];

    let rows = list_rows(&cfg, &findings, &outcomes);

    assert!(
        rows.iter()
            .find(|row| row.id == "allow-broad")
            .is_some_and(|row| row.broad_scope)
    );
}

#[test]
fn render_list_rows_json_records_context_filters_and_rows() {
    let json = sample_list_json_for_contract_test();
    let value = parse_json("list artifact", &json);

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::LIST_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"list\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"allow_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"evidence_count\": 2"));
    assert!(json.contains("\"selector_precision\": 7"));
    assert!(json.contains("\"broad_scope\": false"));
    assert_eq!(
        value.pointer("/filters/kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        value.pointer("/filters/family").and_then(Value::as_str),
        Some("unwrap")
    );
    assert_eq!(
        value.pointer("/filters/owner").and_then(Value::as_str),
        Some("parser")
    );
    assert_eq!(
        value
            .pointer("/filters/classification")
            .and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value.pointer("/filters/path").and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(
        value
            .pointer("/filters/source_package")
            .and_then(Value::as_str),
        Some("allow-core")
    );
    assert_eq!(
        value.pointer("/filters/allow_id").and_then(Value::as_str),
        Some("allow-json")
    );
    assert_eq!(
        value.pointer("/filters/status").and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value.pointer("/filters/expired").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/review_due")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value.pointer("/filters/stale").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/baseline_debt")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/filters/broad_scope")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/missing_evidence")
            .and_then(Value::as_bool),
        Some(false)
    );
}

fn parse_json(name: &str, json: &str) -> Value {
    match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => std::panic::panic_any(format!("{name} should parse as JSON: {err}\n{json}")),
    }
}
