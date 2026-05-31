use crate::artifact_schema_expectations::{
    expected_top_level_required_fields, expected_top_level_schema_properties,
};
use crate::artifact_schema_support::{parse_schema, required_schema_pointer, schema_contracts};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn schema_files_reject_unknown_top_level_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{} schema should reject unknown top-level fields",
            contract.name
        );
    }
}

#[test]
fn schema_files_keep_explicit_top_level_property_sets() {
    for (name, expected) in expected_top_level_schema_properties() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            std::panic::panic_any(format!("{name} schema properties should be an object"));
        };
        let actual = properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level schema properties");
    }
}

#[test]
fn schema_files_keep_explicit_top_level_required_sets() {
    for (name, expected) in expected_top_level_required_fields() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(required) = schema.get("required").and_then(Value::as_array) else {
            std::panic::panic_any(format!("{name} schema required should be an array"));
        };
        let actual = required
            .iter()
            .map(|field| {
                field.as_str().unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{name} schema required entries should be strings"
                    ))
                })
            })
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level required fields");
    }
}

#[test]
fn schema_object_nodes_reject_unknown_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);
        let mut missing = Vec::new();

        collect_object_nodes_missing_additional_properties(&schema, "", &mut missing);

        assert!(
            missing.is_empty(),
            "{} object schemas should set additionalProperties=false at: {}",
            contract.name,
            missing.join(", ")
        );
    }
}

#[test]
fn schema_auxiliary_filter_and_option_objects_keep_optional_members() {
    for (schema_name, pointer) in [
        ("add", "/properties/options"),
        ("list", "/properties/filters"),
        ("propose", "/properties/options"),
        ("worklist", "/$defs/filters"),
    ] {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == schema_name)
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("missing schema contract {schema_name}"))
            });
        let schema = parse_schema(contract.name, contract.schema);
        let object = required_schema_pointer(schema_name, &schema, pointer);

        assert_eq!(
            object.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{schema_name} {pointer} should still reject unknown fields"
        );
        assert!(
            object.get("required").is_none(),
            "{schema_name} {pointer} nested members should stay optional for v1 compatibility"
        );
        assert!(
            object
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| !properties.is_empty()),
            "{schema_name} {pointer} should still lock a known property vocabulary"
        );
    }
}

fn collect_object_nodes_missing_additional_properties(
    value: &Value,
    path: &str,
    missing: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            let has_properties = object.contains_key("properties");
            let is_object_type = object.get("type").and_then(Value::as_str) == Some("object");
            if (has_properties || is_object_type)
                && object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
            {
                missing.push(if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                });
            }
            for (key, child) in object {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{}", json_pointer_escape(key)),
                    missing,
                );
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{index}"),
                    missing,
                );
            }
        }
        _ => {}
    }
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
