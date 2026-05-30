use crate::artifact_contract_support::parse_json_artifact;
use crate::artifact_schema_support::{parse_schema, schema_contracts};
use crate::{add, diff, doctor, explain, list, migrate, propose, prune, worklist};
use serde_json::Value;
use std::collections::BTreeSet;

struct ArtifactSample {
    name: &'static str,
    schema_name: &'static str,
    json: String,
    expected_command: &'static str,
    expected_top_level_keys: &'static [&'static str],
}

#[test]
fn command_artifacts_keep_explicit_top_level_contracts() {
    let samples = command_artifact_samples();
    for sample in samples {
        assert_artifact_contract(&sample);
    }
}

#[test]
fn fixed_command_artifacts_have_top_level_sample_coverage() {
    let mut samples = command_artifact_samples();
    samples.extend(core_artifact_samples());
    assert_sample_coverage_matches_fixed_command_contracts(&samples);
}

#[test]
fn report_artifacts_have_top_level_sample_coverage_for_each_report_command() {
    let samples = core_artifact_samples();
    let expected = allow_report::REPORT_COMMANDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = samples
        .iter()
        .filter(|sample| sample.schema_name == "report")
        .map(|sample| sample.expected_command)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual, expected,
        "the shared report schema should have top-level sample coverage for every producer command"
    );
}

#[test]
fn core_artifacts_keep_explicit_top_level_contracts() {
    for sample in core_artifact_samples() {
        assert_artifact_contract(&sample);
    }
}

fn command_artifact_samples() -> Vec<ArtifactSample> {
    vec![
        ArtifactSample {
            name: "add",
            schema_name: "add",
            json: add::sample_add_json_for_contract_test(),
            expected_command: "add",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "doctor",
            schema_name: "doctor",
            json: doctor::sample_doctor_json_for_contract_test(),
            expected_command: "doctor",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "explain",
            schema_name: "explain",
            json: explain::sample_explain_json_for_contract_test(),
            expected_command: "explain",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "list",
            schema_name: "list",
            json: list::sample_list_json_for_contract_test(),
            expected_command: "list",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "migrate",
            schema_name: "migrate",
            json: migrate::sample_migrate_json_for_contract_test(),
            expected_command: "migrate",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "propose",
            schema_name: "propose",
            json: propose::sample_propose_json_for_contract_test(),
            expected_command: "propose",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "prune",
            schema_name: "prune",
            json: prune::sample_prune_json_for_contract_test(),
            expected_command: "prune",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "worklist",
            schema_name: "worklist",
            json: worklist::sample_worklist_json_for_contract_test(),
            expected_command: "worklist",
            expected_top_level_keys: &[
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
        },
    ]
}

fn core_artifact_samples() -> Vec<ArtifactSample> {
    let audit_report_json = allow_report::render_json_with_context(
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
    let check_report_json = allow_report::render_json_with_context(
        "check",
        &[],
        &[],
        false,
        allow_report::ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
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
    let diff_json = diff::render_diff_json_with_posture(diff_base_json, 0, &[], &[], &[]);
    vec![
        ArtifactSample {
            name: "audit_report",
            schema_name: "report",
            json: audit_report_json,
            expected_command: "audit",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "check_report",
            schema_name: "report",
            json: check_report_json,
            expected_command: "check",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "receipt",
            schema_name: "receipt",
            json: receipt_json,
            expected_command: "check",
            expected_top_level_keys: &[
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
        },
        ArtifactSample {
            name: "diff",
            schema_name: "report",
            json: diff_json,
            expected_command: "diff",
            expected_top_level_keys: &[
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
        },
    ]
}

fn assert_sample_coverage_matches_fixed_command_contracts(samples: &[ArtifactSample]) {
    let expected = schema_contracts()
        .into_iter()
        .filter(|contract| contract.fixed_command.is_some())
        .map(|contract| contract.name)
        .collect::<BTreeSet<_>>();
    let actual = samples
        .iter()
        .map(|sample| sample.schema_name)
        .filter(|schema_name| expected.contains(schema_name))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "every fixed-command artifact contract should have top-level sample coverage"
    );
}

fn schema_contract_by_name(name: &str) -> crate::artifact_schema_support::SchemaContract {
    schema_contracts()
        .into_iter()
        .find(|contract| contract.name == name)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")))
}

fn assert_artifact_contract(sample: &ArtifactSample) {
    let contract = schema_contract_by_name(sample.schema_name);
    let value = parse_json_artifact(
        sample.name,
        &sample.json,
        contract.schema_id,
        sample.expected_command,
    );
    assert_eq!(
        value.get("tool").and_then(Value::as_str),
        Some("cargo-allow"),
        "{} tool",
        sample.name
    );
    assert_top_level_keys(sample.name, &value, sample.expected_top_level_keys);
    assert_schema_covers_sample_top_level_keys(sample.name, contract.name, &value);
    assert_sample_inventory_scanner_matches_schema(sample.name, contract.name, &value);
    assert_string_array_eq(
        sample.name,
        &value,
        "claim_boundary",
        allow_report::CLAIM_BOUNDARY,
    );
    assert_string_array_eq(
        sample.name,
        &value,
        "scanner_limitations",
        allow_report::SCANNER_LIMITATIONS,
    );
}

fn assert_sample_inventory_scanner_matches_schema(name: &str, schema_name: &str, value: &Value) {
    let contract = schema_contract_by_name(schema_name);
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

fn assert_schema_covers_sample_top_level_keys(name: &str, schema_name: &str, value: &Value) {
    let contract = schema_contract_by_name(schema_name);
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
