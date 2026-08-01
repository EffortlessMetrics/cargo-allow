use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn migrate_schema_locks_policy_migration_summary_contract() {
    let schema = parse_schema(
        "migrate",
        include_str!("../../../docs/schemas/migrate.schema.json"),
    );

    assert_required_fields(
        "migrate",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "input",
            "output",
            "summary",
            "closeout",
            "mutation_receipt",
            "notes",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/scanner/const")
            .and_then(Value::as_str),
        Some("policy_migration"),
        "migrate inventory scanner should stay policy_migration"
    );

    let input = required_schema_pointer("migrate", &schema, "/properties/input");
    assert_eq!(
        input.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate input should reject unknown fields"
    );
    assert_required_fields("migrate input", input, &["kind", "path"]);
    assert_enum_equals(
        "migrate",
        &schema,
        "/properties/input/properties/kind/enum",
        &["from", "repo_policy"],
    );
    assert_eq!(
        schema
            .pointer("/properties/input/properties/path/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate input path should be a string"
    );

    let output = required_schema_pointer("migrate", &schema, "/properties/output");
    assert_eq!(
        output.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate output should reject unknown fields"
    );
    assert_required_fields("migrate output", output, &["path", "force"]);
    assert_eq!(
        schema
            .pointer("/properties/output/properties/path/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate output path should be a string"
    );
    assert_eq!(
        schema
            .pointer("/properties/output/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "migrate output force should be boolean"
    );

    let summary = required_schema_pointer("migrate", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate summary should reject unknown fields"
    );
    assert_required_fields(
        "migrate summary",
        summary,
        &[
            "allow_entries",
            "baseline_debt",
            "unsafe_entries",
            "lint_exception_entries",
            "entries_with_evidence",
        ],
    );
    for field in [
        "allow_entries",
        "baseline_debt",
        "unsafe_entries",
        "lint_exception_entries",
        "entries_with_evidence",
        "evidence_entries",
        "entries_with_links",
        "link_entries",
        "broken_evidence_links",
        "unsafe_broken_evidence_links",
        "weak_evidence_references",
        "unsafe_weak_evidence_references",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/properties/summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "migrate summary {field} should be an integer"
        );
    }

    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "migrate evidence repair queues should be an optional array"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_repair_queue"),
        "migrate evidence repair queues should use the queue row definition"
    );
    assert_eq!(
        schema
            .pointer("/properties/follow_up_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "migrate follow_up_queues should be an optional array"
    );
    assert_eq!(
        schema
            .pointer("/properties/follow_up_queues/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/follow_up_queue"),
        "migrate follow_up_queues should use the queue row definition"
    );
    let follow_up_queue = required_schema_pointer("migrate", &schema, "/$defs/follow_up_queue");
    assert_eq!(
        follow_up_queue
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "migrate follow-up queue should reject unknown fields"
    );
    assert_required_fields(
        "migrate follow-up queue",
        follow_up_queue,
        &["signal", "route_kind", "item_kind", "count", "command"],
    );
    assert_enum_equals(
        "migrate follow-up queue signal",
        &schema,
        "/$defs/follow_up_queue/properties/signal/enum",
        &["baseline_debt"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/label/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate follow-up queue label should be a string"
    );
    assert_enum_equals(
        "migrate follow-up queue route kind",
        &schema,
        "/$defs/follow_up_queue/properties/route_kind/enum",
        &["worklist_item_kind"],
    );
    assert_enum_equals(
        "migrate follow-up queue item kind",
        &schema,
        "/$defs/follow_up_queue/properties/item_kind/enum",
        &["baseline_debt"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "migrate follow-up queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "migrate follow-up queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/$defs/follow_up_queue/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate follow-up queue command should be a string"
    );
    let queue = required_schema_pointer("migrate", &schema, "/$defs/evidence_repair_queue");
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "migrate evidence repair queue should reject unknown fields"
    );
    assert_required_fields(
        "migrate evidence repair queue",
        queue,
        &["item_kind", "count", "unsafe_count", "command"],
    );
    assert_enum_equals(
        "migrate evidence repair queue signal",
        &schema,
        "/$defs/evidence_repair_queue/properties/signal/enum",
        &["broken_evidence_links", "weak_evidence_references"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_repair_queue/properties/label/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate evidence repair queue label should be a string"
    );
    assert_enum_equals(
        "migrate evidence repair queue route kind",
        &schema,
        "/$defs/evidence_repair_queue/properties/route_kind/enum",
        &["worklist_item_kind"],
    );
    assert_enum_equals(
        "migrate evidence repair queue item kind",
        &schema,
        "/$defs/evidence_repair_queue/properties/item_kind/enum",
        &["broken_evidence_link", "weak_evidence_reference"],
    );
    for field in ["count", "unsafe_count"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_repair_queue/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "migrate evidence repair queue {field} should be an integer"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_repair_queue/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "migrate evidence repair queue {field} should be non-negative"
        );
    }
    assert_eq!(
        schema
            .pointer("/$defs/evidence_repair_queue/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate evidence repair queue command should be a string"
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_repair_queue/properties/unsafe_command/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate evidence repair queue unsafe_command should be an optional string"
    );
    assert_eq!(
        schema
            .pointer("/properties/closeout/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/closeout"),
        "migrate closeout should use the closeout definition"
    );
    let closeout = required_schema_pointer("migrate", &schema, "/$defs/closeout");
    assert_required_fields(
        "migrate closeout",
        closeout,
        &[
            "preserved",
            "baseline_debt",
            "evidence_debt",
            "next_queues",
            "legacy_retirement",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/notes/type")
            .and_then(Value::as_str),
        Some("string"),
        "migrate notes should be a string"
    );

    let mutation_receipt = required_schema_pointer("migrate", &schema, "/$defs/mutation_receipt");
    assert_eq!(
        mutation_receipt
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "migrate mutation receipt should reject unknown fields"
    );
    assert_required_fields(
        "migrate mutation receipt",
        mutation_receipt,
        &[
            "schema_id",
            "operation",
            "tool_version",
            "repo_root",
            "config_source",
            "ledger_ids",
            "changed_allow_ids",
            "before_fingerprints",
            "after_fingerprints",
            "result",
            "next_commands",
            "claim_boundary",
        ],
    );
    assert_eq!(
        mutation_receipt
            .pointer("/properties/operation/const")
            .and_then(Value::as_str),
        Some("migrate"),
        "migrate mutation receipt operation should be pinned"
    );
}
