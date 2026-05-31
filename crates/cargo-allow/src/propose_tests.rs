use super::*;
use crate::artifact_contract_support::parse_json_artifact;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{Span, StructuralIdentity};
use clap::Parser;
use serde_json::Value;
use std::path::Path;

#[test]
fn clap_parses_propose_force() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "propose",
        "--write",
        "target/proposed.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/propose-summary.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Propose(ProposeArgs {
            write: Some(path),
            force: true,
            summary_format: ProposeSummaryFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if path == Path::new("target/proposed.toml")
            && summary_output == Path::new("target/propose-summary.json")
    ));
}

#[test]
fn clap_rejects_invalid_propose_expiry() {
    let err = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "propose",
        "--expires",
        "not-a-date",
    ]))
    .expect_err("invalid generated baseline expiry should fail closed");

    assert!(
        err.to_string().contains("YYYY-MM-DD"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_rejects_long_propose_expiry() {
    let expires = SimpleDate::today_utc_approx().add_days(121).to_string();
    let err =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "propose", "--expires", &expires]))
            .expect_err("long generated baseline expiry should fail closed");

    assert!(
        err.to_string().contains("within 120 days"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_accepts_maximum_propose_expiry_window() {
    let expires = SimpleDate::today_utc_approx().add_days(120).to_string();
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "propose", "--expires", &expires]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "maximum generated baseline expiry should parse: {err}"
                ))
            });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Propose(ProposeArgs {
            expires: Some(parsed_expires),
            ..
        })) if parsed_expires == expires
    ));
}

#[test]
fn propose_summary_reports_generated_baseline_boundary() {
    let text = render_propose_summary(
        12,
        3,
        1,
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
        ProposeContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(51),
            ),
            kind_filter: Some("panic"),
        },
    );

    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 51")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("kind filter: panic"));
    assert!(text.contains("findings scanned: 12"));
    assert!(text.contains("baseline_debt entries proposed: 3"));
    assert!(text.contains("unsafe baseline_debt entries proposed: 1"));
    assert!(text.contains("owner: unowned"));
    assert!(text.contains("classification: baseline_debt"));
    assert!(text.contains("output: policy/allow.proposed.toml"));
    assert!(text.contains("generated debt still requires human review"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}

#[test]
fn default_baseline_expiry_is_relative_to_current_date() {
    let before = SimpleDate::today_utc_approx().add_days(BASELINE_DEBT_DEFAULT_DAYS);
    let expires = default_baseline_expiry();
    let after = SimpleDate::today_utc_approx().add_days(BASELINE_DEBT_DEFAULT_DAYS);
    let parsed = SimpleDate::parse(&expires)
        .unwrap_or_else(|| std::panic::panic_any("default expiry should be a valid date"));

    assert!(
        before <= parsed && parsed <= after,
        "default baseline expiry should stay relative to the current UTC date"
    );
}

#[test]
fn render_propose_summary_json_records_generated_baseline_boundary() {
    let json = sample_propose_json_for_contract_test();
    let value = parse_json_artifact("propose", &json, allow_report::PROPOSE_SCHEMA_ID, "propose");

    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("git_tracked")
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(51)
    );
    assert_eq!(
        value.pointer("/options/kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        value.pointer("/options/expires").and_then(Value::as_str),
        Some("2026-08-01")
    );
    assert_eq!(
        value
            .pointer("/options/policy_output")
            .and_then(Value::as_str),
        Some("policy/allow.proposed.toml")
    );
    assert_eq!(
        value.pointer("/options/force").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/summary/findings_scanned")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/owner")
            .and_then(Value::as_str),
        Some("unowned")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/classification")
            .and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/reason")
            .and_then(Value::as_str),
        Some("Generated by cargo-allow propose; requires human review.")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/expires")
            .and_then(Value::as_str),
        Some("2026-08-01")
    );
}

#[test]
fn proposed_baseline_entry_uses_current_created_date() {
    let before = SimpleDate::today_utc_approx();
    let entry = entry_from_finding(
        &Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".into(),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("rust", "method_call"),
            message: "test finding".to_string(),
        },
        1,
        "2026-08-01",
    );
    let after = SimpleDate::today_utc_approx();
    let created = entry
        .lifecycle
        .created
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("entry should have a valid created date"));

    assert!(
        before <= created && created <= after,
        "generated baseline entries should use the current UTC date"
    );
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
