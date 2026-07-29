use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, assert_schema_type_equals, parse_schema,
    required_schema_pointer,
};
use serde_json::Value;

#[test]
fn doctor_schema_locks_setup_artifact_contract() {
    let schema = parse_schema(
        "doctor",
        include_str!("../../../docs/schemas/doctor.schema.json"),
    );

    assert_required_fields(
        "doctor",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "root",
            "config",
            "inventory",
            "federation",
            "evidence_repair_queues",
        ],
    );
    let root = required_schema_pointer("doctor", &schema, "/properties/root");
    assert_eq!(
        root.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor root should reject unknown fields"
    );
    assert_required_fields("doctor root", root, &["path", "discovery"]);
    assert_enum_equals(
        "doctor",
        &schema,
        "/properties/root/properties/discovery/enum",
        &[
            "explicit_root",
            "nearest_git_root",
            "current_directory_fallback",
        ],
    );

    let config = required_schema_pointer("doctor", &schema, "/properties/config");
    assert_eq!(
        config.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor config should reject unknown fields"
    );
    assert_required_fields("doctor config", config, &["found"]);
    assert_eq!(
        schema
            .pointer("/properties/config/properties/found/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "doctor config found should be boolean"
    );
    assert_schema_type_equals(
        "doctor config path",
        &schema,
        "/properties/config/properties/path/type",
        &["string"],
    );
    assert_schema_type_equals(
        "doctor config valid",
        &schema,
        "/properties/config/properties/valid/type",
        &["boolean"],
    );
    assert_schema_type_equals(
        "doctor config diagnostic",
        &schema,
        "/properties/config/properties/diagnostic/type",
        &["string"],
    );
    assert_eq!(
        schema
            .pointer("/properties/config/properties/suggested_init_command/type")
            .and_then(Value::as_str),
        Some("string"),
        "doctor config suggested_init_command should be an optional string"
    );
    for field in ["broken_evidence_links", "weak_evidence_references"] {
        assert_eq!(
            schema
                .pointer(&format!("/properties/config/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "doctor config {field} should be an integer"
        );
        assert_eq!(
            schema
                .pointer(&format!("/properties/config/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "doctor config {field} minimum"
        );
    }
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "doctor evidence_repair_queues should be a top-level array"
    );
    let queue = required_schema_pointer(
        "doctor",
        &schema,
        "/properties/evidence_repair_queues/items",
    );
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor evidence repair queue rows should reject unknown fields"
    );
    assert_required_fields(
        "doctor evidence repair queue",
        queue,
        &["signal", "count", "command"],
    );
    assert_enum_equals(
        "doctor evidence repair queue signal",
        &schema,
        "/properties/evidence_repair_queues/items/properties/signal/enum",
        &["broken_evidence_links", "weak_evidence_references"],
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/label/type",)
            .and_then(Value::as_str),
        Some("string"),
        "doctor evidence repair queue label should be a string"
    );
    assert_enum_equals(
        "doctor evidence repair queue route kind",
        &schema,
        "/properties/evidence_repair_queues/items/properties/route_kind/enum",
        &["worklist_item_kind", "worklist_filter"],
    );
    assert_enum_equals(
        "doctor evidence repair queue worklist filter",
        &schema,
        "/properties/evidence_repair_queues/items/properties/worklist_filter/enum",
        &["broken_evidence", "weak_evidence"],
    );
    assert_enum_equals(
        "doctor evidence repair queue item kind",
        &schema,
        "/properties/evidence_repair_queues/items/properties/item_kind/enum",
        &["broken_evidence_link", "weak_evidence_reference"],
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/type",)
            .and_then(Value::as_str),
        Some("integer"),
        "doctor evidence repair queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/minimum",)
            .and_then(Value::as_u64),
        Some(0),
        "doctor evidence repair queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/command/type",)
            .and_then(Value::as_str),
        Some("string"),
        "doctor evidence repair queue command should be a string"
    );

    assert_eq!(
        schema
            .pointer("/properties/inventory/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/inventory"),
        "doctor inventory should use the inventory schema"
    );
    assert_required_fields(
        "doctor inventory",
        required_schema_pointer("doctor", &schema, "/$defs/inventory"),
        &["scope", "scanner", "source", "files_scanned"],
    );
}
