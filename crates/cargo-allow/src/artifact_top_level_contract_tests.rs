use crate::artifact_contract_samples::{
    ArtifactSample, command_artifact_samples, core_artifact_samples,
};
use crate::artifact_contract_support::parse_json_artifact;
use crate::artifact_schema_support::{parse_schema, schema_contracts};
use serde_json::Value;
use std::collections::BTreeSet;

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

#[test]
fn artifact_samples_keep_nested_keys_covered_by_schemas() {
    let mut samples = command_artifact_samples();
    samples.extend(core_artifact_samples());

    for sample in samples {
        assert_artifact_nested_schema_contract(&sample);
    }
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
        allow_report::claim_boundary_for_schema_id(contract.schema_id),
    );
    assert_string_array_eq(
        sample.name,
        &value,
        "scanner_limitations",
        allow_report::scanner_limitations_for_schema_id(contract.schema_id),
    );
}

fn assert_artifact_nested_schema_contract(sample: &ArtifactSample) {
    let contract = schema_contract_by_name(sample.schema_name);
    let value = parse_json_artifact(
        sample.name,
        &sample.json,
        contract.schema_id,
        sample.expected_command,
    );
    let schema = parse_schema(contract.name, contract.schema);

    if let Err(message) = crate::artifact_sample_schema_support::schema_covers_sample_value(
        &schema, &schema, &value, "$",
    ) {
        std::panic::panic_any(format!(
            "{} sample emitted JSON outside {} schema: {message}",
            sample.name, contract.name
        ));
    }
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
