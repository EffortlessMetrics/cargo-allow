use crate::artifact_contract_support::parse_json_artifact;
use crate::artifact_schema_support::{parse_schema, schema_contracts};
use crate::{add, diff, doctor, explain, list, migrate, propose, prune, worklist};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn command_artifacts_keep_explicit_top_level_contracts() {
    assert_artifact_contract(
        "add",
        &add::sample_add_json_for_contract_test(),
        allow_report::ADD_SCHEMA_ID,
        "add",
        &[
            "allow_entry",
            "claim_boundary",
            "command",
            "inventory",
            "options",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "selected_finding",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "doctor",
        &doctor::sample_doctor_json_for_contract_test(),
        allow_report::DOCTOR_SCHEMA_ID,
        "doctor",
        &[
            "claim_boundary",
            "command",
            "config",
            "inventory",
            "root",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "tool",
        ],
    );
    assert_artifact_contract(
        "explain",
        &explain::sample_explain_json_for_contract_test(),
        allow_report::EXPLAIN_SCHEMA_ID,
        "explain",
        &[
            "allow_entry",
            "claim_boundary",
            "command",
            "current_findings",
            "evidence_references",
            "inventory",
            "match_outcomes",
            "next",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "list",
        &list::sample_list_json_for_contract_test(),
        allow_report::LIST_SCHEMA_ID,
        "list",
        &[
            "allow_entries",
            "claim_boundary",
            "command",
            "filters",
            "inventory",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "migrate",
        &migrate::sample_migrate_json_for_contract_test(),
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        &[
            "claim_boundary",
            "command",
            "input",
            "inventory",
            "notes",
            "output",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "propose",
        &propose::sample_propose_json_for_contract_test(),
        allow_report::PROPOSE_SCHEMA_ID,
        "propose",
        &[
            "claim_boundary",
            "command",
            "generated_entry_defaults",
            "inventory",
            "options",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "prune",
        &prune::sample_prune_json_for_contract_test(),
        allow_report::PRUNE_SCHEMA_ID,
        "prune",
        &[
            "claim_boundary",
            "command",
            "inventory",
            "mode",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "stale_entries",
            "summary",
            "tool",
        ],
    );
    assert_artifact_contract(
        "worklist",
        &worklist::sample_worklist_json_for_contract_test(),
        allow_report::WORKLIST_SCHEMA_ID,
        "worklist",
        &[
            "claim_boundary",
            "command",
            "filters",
            "inventory",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "summary",
            "tool",
            "work_items",
        ],
    );
}

#[test]
fn core_artifacts_keep_explicit_top_level_contracts() {
    let report_json = allow_report::render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        allow_report::ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
            None,
        ),
    );
    assert_artifact_contract(
        "report",
        &report_json,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
        &[
            "claim_boundary",
            "command",
            "failed",
            "findings",
            "inventory",
            "outcomes",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "status",
            "summary",
            "tool",
            "trend",
        ],
    );

    let receipt_json = allow_report::render_receipt_with_context(
        "check",
        &[],
        false,
        allow_report::ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
    );
    assert_artifact_contract(
        "receipt",
        &receipt_json,
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
        &[
            "claim_boundary",
            "command",
            "counts",
            "failed",
            "inventory",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "status",
            "tool",
        ],
    );

    let diff_base_json = allow_report::render_json_with_context(
        "diff",
        &[],
        &[],
        false,
        allow_report::ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(8),
            None,
        ),
    );
    let diff_json = diff::render_diff_json_with_posture(diff_base_json, &[], &[], &[]);
    assert_artifact_contract(
        "diff",
        &diff_json,
        allow_report::REPORT_SCHEMA_ID,
        "diff",
        &[
            "claim_boundary",
            "command",
            "diff",
            "failed",
            "findings",
            "inventory",
            "outcomes",
            "scanner_limitations",
            "schema_id",
            "schema_version",
            "status",
            "summary",
            "tool",
            "trend",
        ],
    );
}

fn assert_artifact_contract(
    name: &str,
    json: &str,
    expected_schema_id: &str,
    expected_command: &str,
    expected_top_level_keys: &[&str],
) {
    let value = parse_json_artifact(name, json, expected_schema_id, expected_command);
    assert_eq!(
        value.get("tool").and_then(Value::as_str),
        Some("cargo-allow"),
        "{name} tool"
    );
    assert_top_level_keys(name, &value, expected_top_level_keys);
    assert_schema_covers_sample_top_level_keys(name, expected_schema_id, &value);
    assert_sample_inventory_scanner_matches_schema(name, expected_schema_id, &value);
    assert_string_array_eq(name, &value, "claim_boundary", allow_report::CLAIM_BOUNDARY);
    assert_string_array_eq(
        name,
        &value,
        "scanner_limitations",
        allow_report::SCANNER_LIMITATIONS,
    );
}

fn assert_sample_inventory_scanner_matches_schema(
    name: &str,
    expected_schema_id: &str,
    value: &Value,
) {
    let contract = schema_contracts()
        .into_iter()
        .find(|contract| contract.schema_id == expected_schema_id)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "missing schema contract for {name} schema_id {expected_schema_id}"
            ))
        });
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(contract.inventory_scanner),
        "{name} inventory scanner"
    );
}

fn assert_top_level_keys(name: &str, value: &Value, expected: &[&str]) {
    let Some(object) = value.as_object() else {
        std::panic::panic_any(format!("{name} artifact should be a JSON object"));
    };
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{name} top-level keys");
}

fn assert_schema_covers_sample_top_level_keys(name: &str, expected_schema_id: &str, value: &Value) {
    let contract = schema_contracts()
        .into_iter()
        .find(|contract| contract.schema_id == expected_schema_id)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "missing schema contract for {name} schema_id {expected_schema_id}"
            ))
        });
    let schema = parse_schema(contract.name, contract.schema);
    let Some(sample) = value.as_object() else {
        std::panic::panic_any(format!("{name} sample should be a JSON object"));
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        std::panic::panic_any(format!("{name} schema properties should be an object"));
    };
    let allowed = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = sample.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let unknown = actual.difference(&allowed).copied().collect::<Vec<_>>();
    assert!(
        unknown.is_empty(),
        "{name} sample emitted top-level keys absent from schema properties: {}",
        unknown.join(", ")
    );

    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} schema required should be an array"));
    };
    let missing = required
        .iter()
        .map(|field| {
            field.as_str().unwrap_or_else(|| {
                std::panic::panic_any(format!("{name} schema required entries should be strings"))
            })
        })
        .filter(|field| !actual.contains(field))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "{name} sample omitted schema-required top-level keys: {}",
        missing.join(", ")
    );
}

fn assert_string_array_eq(name: &str, value: &Value, field: &str, expected: &[&str]) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {field} should be an array"));
    };
    let actual = items
        .iter()
        .map(|item| {
            item.as_str().unwrap_or_else(|| {
                std::panic::panic_any(format!("{name} {field} entries should be strings"))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{name} {field}");
}
