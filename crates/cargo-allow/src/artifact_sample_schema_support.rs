use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn schema_covers_sample_value(
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), String> {
    let schema = resolve_local_schema_ref(root_schema, schema, path)?;

    if let Some(branches) = schema.get("allOf").and_then(Value::as_array) {
        for (index, branch) in branches.iter().enumerate() {
            schema_covers_sample_value(
                root_schema,
                branch,
                value,
                &format!("{path}.allOf[{index}]"),
            )?;
        }
    }

    validate_conditional_schema(root_schema, schema, value, path)?;

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

            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
                let allowed = properties
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                    let unknown = actual.difference(&allowed).copied().collect::<Vec<_>>();
                    if !unknown.is_empty() {
                        return Err(format!(
                            "{path} has keys absent from schema properties: {}",
                            unknown.join(", ")
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
            } else if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false)
                && !object.is_empty()
            {
                return Err(format!(
                    "{path} has object keys but schema allows no properties"
                ));
            }
        }
        Value::Array(items) => {
            if let Some(contains_schema) = schema.get("contains") {
                let mut matched = false;
                for item in items {
                    if schema_covers_sample_value(root_schema, contains_schema, item, path).is_ok()
                    {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return Err(format!("{path} did not contain a value matching contains"));
                }
            }
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

fn validate_conditional_schema(
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
) -> Result<(), String> {
    let Some(if_schema) = schema.get("if") else {
        return Ok(());
    };

    if schema_covers_sample_value(root_schema, if_schema, value, &format!("{path}.if")).is_ok() {
        if let Some(then_schema) = schema.get("then") {
            schema_covers_sample_value(root_schema, then_schema, value, &format!("{path}.then"))?;
        }
    } else if let Some(else_schema) = schema.get("else") {
        schema_covers_sample_value(root_schema, else_schema, value, &format!("{path}.else"))?;
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

pub(crate) fn supported_schema_patterns() -> BTreeSet<String> {
    ["^cargo-allow ", "^work-[a-z0-9-]+-[0-9]{4}$"]
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect()
}

pub(crate) fn collect_schema_patterns(value: &Value, patterns: &mut BTreeSet<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_schema_support::{parse_schema, schema_contracts};

    #[test]
    fn artifact_sample_validator_enforces_contains_constraints() {
        let schema = json_value(r#"{"type":"array","contains":{"const":"source_tree_inventory"}}"#);
        let valid = json_value(r#"["source_tree_inventory","source_syntax_only"]"#);
        let invalid = json_value(r#"["source_syntax_only"]"#);

        assert!(
            schema_covers_sample_value(&schema, &schema, &valid, "$").is_ok(),
            "sample validator should accept arrays that satisfy contains"
        );
        assert!(
            schema_covers_sample_value(&schema, &schema, &invalid, "$").is_err(),
            "sample validator should reject arrays that miss contains"
        );
    }

    #[test]
    fn artifact_sample_validator_enforces_conditional_all_of_constraints() {
        let schema = json_value(
            r#"{
                "type":"object",
                "allOf":[
                    {
                        "if":{"required":["diff"]},
                        "then":{"properties":{"command":{"const":"diff"}}}
                    }
                ]
            }"#,
        );
        let valid = json_value(r#"{"command":"diff","diff":{}}"#);
        let invalid = json_value(r#"{"command":"check","diff":{}}"#);

        assert!(
            schema_covers_sample_value(&schema, &schema, &valid, "$").is_ok(),
            "sample validator should accept diff artifacts with diff command"
        );
        assert!(
            schema_covers_sample_value(&schema, &schema, &invalid, "$").is_err(),
            "sample validator should reject diff artifacts with non-diff command"
        );
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

    fn json_value(input: &str) -> Value {
        serde_json::from_str(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test JSON should parse: {err}")))
    }
}
