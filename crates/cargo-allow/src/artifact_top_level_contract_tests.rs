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

#[test]
fn artifact_samples_keep_nested_keys_covered_by_schemas() {
    let mut samples = command_artifact_samples();
    samples.extend(core_artifact_samples());

    for sample in samples {
        assert_artifact_nested_schema_contract(&sample);
    }
}

#[test]
fn artifact_sample_validator_covers_every_schema_pattern() {
    let mut actual = BTreeSet::new();
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);
        collect_schema_patterns(&schema, &mut actual);
    }

    let expected = supported_schema_patterns();
    assert_eq!(
        actual, expected,
        "artifact sample validation should explicitly support every JSON Schema pattern"
    );
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

fn assert_artifact_nested_schema_contract(sample: &ArtifactSample) {
    let contract = schema_contract_by_name(sample.schema_name);
    let value = parse_json_artifact(
        sample.name,
        &sample.json,
        contract.schema_id,
        sample.expected_command,
    );
    let schema = parse_schema(contract.name, contract.schema);

    if let Err(message) = schema_covers_sample_value(&schema, &schema, &value, "$") {
        std::panic::panic_any(format!(
            "{} sample emitted JSON outside {} schema: {message}",
            sample.name, contract.name
        ));
    }
}

fn schema_covers_sample_value(
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), String> {
    let schema = resolve_local_schema_ref(root_schema, schema, path)?;

    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        let mut errors = Vec::new();
        for branch in branches {
            match schema_covers_sample_value(root_schema, branch, value, path) {
                Ok(()) => return Ok(()),
                Err(err) => errors.push(err),
            }
        }
        return Err(format!(
            "{path} did not match any anyOf branch: {}",
            errors.join("; ")
        ));
    }

    validate_sample_value_constraints(schema, value, path)?;

    match value {
        Value::Object(object) => {
            let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
                return if object.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "{path} has object keys but schema has no properties"
                    ))
                };
            };

            let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
            let allowed = properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let unknown = actual.difference(&allowed).copied().collect::<Vec<_>>();
            if !unknown.is_empty() {
                return Err(format!(
                    "{path} has keys absent from schema properties: {}",
                    unknown.join(", ")
                ));
            }

            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                let missing = required
                    .iter()
                    .map(|field| {
                        field.as_str().ok_or_else(|| {
                            format!("{path} schema required entries should be strings")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|field| !object.contains_key(*field))
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    return Err(format!(
                        "{path} is missing schema-required keys: {}",
                        missing.join(", ")
                    ));
                }
            }

            for (key, child) in object {
                if let Some(child_schema) = properties.get(key) {
                    schema_covers_sample_value(
                        root_schema,
                        child_schema,
                        child,
                        &format!("{path}.{}", key),
                    )?;
                }
            }
        }
        Value::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    schema_covers_sample_value(
                        root_schema,
                        item_schema,
                        item,
                        &format!("{path}[{index}]"),
                    )?;
                }
            }
        }
        Value::Null => {
            if schema.get("type").and_then(Value::as_str) == Some("null") {
                return Ok(());
            }
            if schema
                .get("type")
                .and_then(Value::as_array)
                .is_some_and(|types| types.iter().any(|item| item.as_str() == Some("null")))
            {
                return Ok(());
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_sample_value_constraints(
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), String> {
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(format!(
                "{path} has value {}, expected const {}",
                value, expected
            ));
        }
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|expected| expected == value) {
            return Err(format!("{path} has value {value}, outside schema enum"));
        }
    }

    if !schema_accepts_value_type(schema, value) {
        return Err(format!(
            "{path} has JSON type {}, outside schema type",
            json_value_type(value)
        ));
    }

    if let (Some(value), Some(minimum)) = (
        value.as_f64(),
        schema.get("minimum").and_then(Value::as_f64),
    ) {
        if value < minimum {
            return Err(format!(
                "{path} has numeric value {value}, below minimum {minimum}"
            ));
        }
    }

    if let (Some(value), Some(min_length)) = (
        value.as_str(),
        schema.get("minLength").and_then(Value::as_u64),
    ) {
        if value.chars().count() < min_length as usize {
            return Err(format!(
                "{path} has string shorter than minLength {min_length}"
            ));
        }
    }

    if let (Some(value), Some(pattern)) = (
        value.as_str(),
        schema.get("pattern").and_then(Value::as_str),
    ) {
        if !sample_string_matches_supported_pattern(value, pattern) {
            return Err(format!(
                "{path} has string {value:?}, outside supported schema pattern {pattern:?}"
            ));
        }
    }

    Ok(())
}

fn schema_accepts_value_type(schema: &Value, value: &Value) -> bool {
    let Some(schema_type) = schema.get("type") else {
        return true;
    };

    if let Some(schema_type) = schema_type.as_str() {
        return json_value_matches_schema_type(value, schema_type);
    }
    schema_type.as_array().is_none_or(|types| {
        types.iter().any(|schema_type| {
            schema_type
                .as_str()
                .is_some_and(|schema_type| json_value_matches_schema_type(value, schema_type))
        })
    })
}

fn json_value_matches_schema_type(value: &Value, schema_type: &str) -> bool {
    match schema_type {
        "array" => value.is_array(),
        "boolean" => value.is_boolean(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        "object" => value.is_object(),
        "string" => value.is_string(),
        _ => true,
    }
}

fn json_value_type(value: &Value) -> &'static str {
    match value {
        Value::Array(_) => "array",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            "integer"
        }
        Value::Number(_) => "number",
        Value::Object(_) => "object",
        Value::String(_) => "string",
    }
}

fn sample_string_matches_supported_pattern(value: &str, pattern: &str) -> bool {
    match pattern {
        "^cargo-allow " => value.starts_with("cargo-allow "),
        "^work-[a-z0-9-]+-[0-9]{4}$" => sample_string_matches_work_item_id(value),
        _ => std::panic::panic_any(format!("unsupported schema pattern {pattern:?}")),
    }
}

fn sample_string_matches_work_item_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("work-") else {
        return false;
    };
    let Some((kind, number)) = rest.rsplit_once('-') else {
        return false;
    };
    !kind.is_empty()
        && kind
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && number.len() == 4
        && number.chars().all(|ch| ch.is_ascii_digit())
}

fn supported_schema_patterns() -> BTreeSet<String> {
    ["^cargo-allow ", "^work-[a-z0-9-]+-[0-9]{4}$"]
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect()
}

fn collect_schema_patterns(value: &Value, patterns: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
                patterns.insert(pattern.to_string());
            }
            for child in object.values() {
                collect_schema_patterns(child, patterns);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_schema_patterns(child, patterns);
            }
        }
        _ => {}
    }
}

fn resolve_local_schema_ref<'a>(
    root_schema: &'a Value,
    schema: &'a Value,
    path: &str,
) -> Result<&'a Value, String> {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return Ok(schema);
    };
    let Some(pointer) = reference.strip_prefix('#') else {
        return Err(format!("{path} schema uses non-local ref {reference}"));
    };
    root_schema
        .pointer(pointer)
        .ok_or_else(|| format!("{path} schema ref {reference} did not resolve"))
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
