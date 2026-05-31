use crate::artifact_schema_support::{parse_schema, required_schema_pointer};
use serde_json::Value;

#[test]
fn schema_source_location_fields_are_one_based_when_present() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    for pointer in [
        "/$defs/finding/properties/line",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
        "/$defs/selector/properties/line_hint",
    ] {
        assert_schema_minimum("common", &common, pointer, 1);
    }

    let add = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));
    for pointer in [
        "/$defs/selector/properties/line_hint",
        "/$defs/span/properties/line",
        "/$defs/span/properties/column",
        "/$defs/finding/properties/line",
        "/$defs/finding/properties/column",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
    ] {
        assert_schema_minimum("add", &add, pointer, 1);
    }

    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );
    for pointer in [
        "/$defs/selector/properties/line_hint",
        "/$defs/current_finding/properties/line",
        "/$defs/current_finding/properties/column",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
        "/$defs/span/properties/line",
        "/$defs/span/properties/column",
    ] {
        assert_schema_minimum("explain", &explain, pointer, 1);
    }

    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );
    assert_schema_minimum("report", &report, "/$defs/finding/properties/line", 1);
}

fn assert_schema_minimum(name: &str, schema: &Value, pointer: &str, expected: u64) {
    assert_eq!(
        required_schema_pointer(name, schema, pointer)
            .get("minimum")
            .and_then(Value::as_u64),
        Some(expected),
        "{name} {pointer} minimum"
    );
}
