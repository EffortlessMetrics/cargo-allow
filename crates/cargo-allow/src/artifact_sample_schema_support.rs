use serde_json::Value;
use std::collections::BTreeSet;

use crate::artifact_sample_schema_patterns::sample_string_matches_supported_pattern;

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
    use crate::artifact_sample_schema_patterns::{
        collect_schema_patterns, supported_schema_patterns,
    };
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
    fn artifact_sample_validator_reports_object_shape_errors() {
        let any_of_schema = json_value(r#"{"anyOf":[{"const":"allow"},{"const":"audit"}]}"#);
        assert_eq!(
            schema_covers_sample_value(&any_of_schema, &any_of_schema, &json_value(r#""check""#), "$.mode"),
            Err("$.mode did not match any anyOf branch: $.mode has value \"check\", expected const \"allow\"; $.mode has value \"check\", expected const \"audit\"".to_string())
        );

        let required_schema = json_value(r#"{"type":"object","required":["name","kind"]}"#);
        assert_eq!(
            schema_covers_sample_value(
                &required_schema,
                &required_schema,
                &json_value(r#"{"kind":"audit"}"#),
                "$"
            ),
            Err("$ is missing schema-required keys: name".to_string())
        );

        let additional_properties_schema = json_value(
            r#"{
                "type":"object",
                "properties":{"known":{"type":"string"}},
                "additionalProperties":false
            }"#,
        );
        assert_eq!(
            schema_covers_sample_value(
                &additional_properties_schema,
                &additional_properties_schema,
                &json_value(r#"{"known":"ok","extra":true}"#),
                "$"
            ),
            Err("$ has keys absent from schema properties: extra".to_string())
        );

        let no_properties_schema = json_value(r#"{"type":"object","additionalProperties":false}"#);
        assert_eq!(
            schema_covers_sample_value(
                &no_properties_schema,
                &no_properties_schema,
                &json_value(r#"{"extra":true}"#),
                "$"
            ),
            Err("$ has object keys but schema allows no properties".to_string())
        );
    }

    #[test]
    fn artifact_sample_validator_reports_array_and_scalar_constraint_errors() {
        let contains_schema = json_value(r#"{"type":"array","contains":{"const":"required"}}"#);
        let path = "$";
        let err = schema_covers_sample_value(
            &contains_schema,
            &contains_schema,
            &json_value(r#"["other"]"#),
            path,
        )
        .expect_err("array missing contains match should fail validation");
        assert_eq!(
            err,
            format!("{path} did not contain a value matching contains")
        );

        let any_of_schema = json_value(r#"{"anyOf":[{"const":"allow"},{"const":"audit"}]}"#);
        let path = "$.mode";
        let err = schema_covers_sample_value(
            &any_of_schema,
            &any_of_schema,
            &json_value(r#""check""#),
            path,
        )
        .expect_err("value outside anyOf branches should fail validation");
        assert_eq!(
            err,
            format!(
                "{path} did not match any anyOf branch: {path} has value \"check\", expected const \"allow\"; {path} has value \"check\", expected const \"audit\""
            )
        );

        let required_schema = json_value(r#"{"type":"object","required":["name","kind"]}"#);
        let path = "$";
        let err = schema_covers_sample_value(
            &required_schema,
            &required_schema,
            &json_value(r#"{"kind":"audit"}"#),
            path,
        )
        .expect_err("object missing required keys should fail validation");
        assert_eq!(
            err,
            format!("{path} is missing schema-required keys: name")
        );

        let additional_properties_schema = json_value(
            r#"{
                "type":"object",
                "properties":{"known":{"type":"string"}},
                "additionalProperties":false
            }"#,
        );
        let path = "$";
        let err = schema_covers_sample_value(
            &additional_properties_schema,
            &additional_properties_schema,
            &json_value(r#"{"known":"ok","extra":true}"#),
            path,
        )
        .expect_err("object with unknown keys should fail validation");
        assert_eq!(
            err,
            format!("{path} has keys absent from schema properties: extra")
        );

        let no_properties_schema = json_value(r#"{"type":"object","additionalProperties":false}"#);
        let path = "$";
        let err = schema_covers_sample_value(
            &no_properties_schema,
            &no_properties_schema,
            &json_value(r#"{"extra":true}"#),
            path,
        )
        .expect_err("object with disallowed properties should fail validation");
        assert_eq!(
            err,
            format!("{path} has object keys but schema allows no properties")
        );

        let const_schema = json_value(r#"{"const":"expected"}"#);
        let path = "$.command";
        let err = schema_covers_sample_value(
            &const_schema,
            &const_schema,
            &json_value(r#""actual""#),
            path,
        )
        .expect_err("value outside const should fail validation");
        assert_eq!(
            err,
            format!("{path} has value \"actual\", expected const \"expected\"")
        );

        let enum_schema = json_value(r#"{"enum":["audit","check"]}"#);
        let path = "$.mode";
        let err = schema_covers_sample_value(
            &enum_schema,
            &enum_schema,
            &json_value(r#""doctor""#),
            path,
        )
        .expect_err("value outside enum should fail validation");
        assert_eq!(
            err,
            format!("{path} has value \"doctor\", outside schema enum")
        );

        let type_schema = json_value(r#"{"type":"string"}"#);
        let path = "$.id";
        let err = schema_covers_sample_value(
            &type_schema,
            &type_schema,
            &json_value(r#"42"#),
            path,
        )
        .expect_err("value outside schema type should fail validation");
        assert_eq!(
            err,
            format!("{path} has JSON type integer, outside schema type")
        );

        let minimum_schema = json_value(r#"{"type":"number","minimum":10}"#);
        let path = "$.count";
        let err = schema_covers_sample_value(
            &minimum_schema,
            &minimum_schema,
            &json_value(r#"3"#),
            path,
        )
        .expect_err("value below minimum should fail validation");
        assert_eq!(
            err,
            format!("{path} has numeric value 3, below minimum 10")
        );

        let min_length_schema = json_value(r#"{"type":"string","minLength":4}"#);
        let path = "$.id";
        let err = schema_covers_sample_value(
            &min_length_schema,
            &min_length_schema,
            &json_value(r#""abc""#),
            path,
        )
        .expect_err("string shorter than minLength should fail validation");
        assert_eq!(
            err,
            format!("{path} has string shorter than minLength 4")
        );

        let pattern_schema = json_value(r#"{"type":"string","pattern":"^cargo-allow "}"#);
        let path = "$.command";
        let err = schema_covers_sample_value(
            &pattern_schema,
            &pattern_schema,
            &json_value(r#""cargo check""#),
            path,
        )
        .expect_err("string outside supported pattern should fail validation");
        assert_eq!(
            err,
            format!(
                "{path} has string \"cargo check\", outside supported schema pattern \"^cargo-allow \""
            )
        );
    }

    #[test]
    fn artifact_sample_validator_reports_non_local_refs() {
        let schema = json_value(r#"{"$ref":"https://example.test/schema.json"}"#);
        let path = "$";
        let reference = "https://example.test/schema.json";

        let err = schema_covers_sample_value(&schema, &schema, &json_value(r#"null"#), path)
            .expect_err("non-local schema refs should fail validation");
        assert_eq!(
            err,
            format!("{path} schema uses non-local ref {reference}")
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
