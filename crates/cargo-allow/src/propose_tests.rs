use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{Span, StructuralIdentity};
use clap::Parser;
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
fn propose_summary_reports_generated_baseline_boundary() {
    let text = render_propose_summary(
        12,
        3,
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
    );

    assert!(text.contains("findings scanned: 12"));
    assert!(text.contains("baseline_debt entries proposed: 3"));
    assert!(text.contains("owner: unowned"));
    assert!(text.contains("classification: baseline_debt"));
    assert!(text.contains("output: policy/allow.proposed.toml"));
    assert!(text.contains("generated debt still requires human review"));
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

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::PROPOSE_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"propose\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 51"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"policy_output\": \"policy/allow.proposed.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"findings_scanned\": 12"));
    assert!(json.contains("\"baseline_debt_entries_proposed\": 3"));
    assert!(json.contains("\"owner\": \"unowned\""));
    assert!(json.contains("\"classification\": \"baseline_debt\""));
    assert!(json.contains("\"repository_code_not_executed\""));
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

#[test]
fn propose_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/propose.schema.json");

    assert!(schema.contains(allow_report::PROPOSE_SCHEMA_ID));
    assert!(schema.contains("\"options\""));
    assert!(schema.contains("\"policy_output\""));
    assert!(schema.contains("\"baseline_debt_entries_proposed\""));
    assert!(schema.contains("\"generated_entry_defaults\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
