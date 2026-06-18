use crate::artifact_schema_support::{
    assert_enum_equals, assert_required_fields, parse_schema, required_schema_pointer,
};
use serde_json::Value;

#[test]
fn report_schema_allows_optional_policy_baseline_debt_summary_count() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let policy_baseline_debt = required_schema_pointer(
        "report",
        &schema,
        "/$defs/summary/properties/policy_baseline_debt",
    );
    assert_eq!(
        policy_baseline_debt.get("type").and_then(Value::as_str),
        Some("integer"),
        "report policy_baseline_debt count type"
    );
    assert_eq!(
        policy_baseline_debt.get("minimum").and_then(Value::as_u64),
        Some(0),
        "report policy_baseline_debt count minimum"
    );
}

#[test]
fn report_schema_locks_top_level_status_vocabulary() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report status",
        &schema,
        "/properties/status/enum",
        allow_report::ARTIFACT_STATUSES,
    );
}

#[test]
fn report_schema_locks_report_command_producers() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_enum_equals(
        "report command",
        &schema,
        "/properties/command/enum",
        allow_report::REPORT_COMMANDS,
    );
}

#[test]
fn report_schema_allows_optional_broken_evidence_link_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/broken_evidence_links",
        "/$defs/trend/properties/broken_evidence_links",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_weak_evidence_reference_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/weak_evidence_references",
        "/$defs/trend/properties/weak_evidence_references",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_evidence_repair_queues() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/type")
            .and_then(Value::as_str),
        Some("array"),
        "report evidence repair queues should be an optional array"
    );
    let queue = required_schema_pointer(
        "report",
        &schema,
        "/properties/evidence_repair_queues/items",
    );
    assert_eq!(
        queue.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "report evidence repair queue rows should reject unknown fields"
    );
    assert_required_fields(
        "report evidence repair queue",
        queue,
        &["signal", "count", "command"],
    );
    assert_enum_equals(
        "report evidence repair queue signal",
        &schema,
        "/properties/evidence_repair_queues/items/properties/signal/enum",
        &[
            "broken_evidence_links",
            "missing_evidence",
            "weak_evidence_references",
            "occurrence_headroom",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/label/type")
            .and_then(Value::as_str),
        Some("string"),
        "report evidence repair queue label should be a string"
    );
    assert_enum_equals(
        "report evidence repair queue route kind",
        &schema,
        "/properties/evidence_repair_queues/items/properties/route_kind/enum",
        &["worklist_item_kind", "worklist_filter"],
    );
    assert_enum_equals(
        "report evidence repair queue item kind",
        &schema,
        "/properties/evidence_repair_queues/items/properties/item_kind/enum",
        &[
            "broken_evidence_link",
            "missing_evidence",
            "weak_evidence_reference",
            "occurrence_headroom",
        ],
    );
    assert_enum_equals(
        "report evidence repair queue worklist filter",
        &schema,
        "/properties/evidence_repair_queues/items/properties/worklist_filter/enum",
        &["broken_evidence", "missing_evidence", "weak_evidence"],
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "report evidence repair queue count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "report evidence repair queue count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/properties/evidence_repair_queues/items/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "report evidence repair queue command should be a string"
    );
}

#[test]
fn report_schema_allows_optional_audit_remediation_roadmap() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/audit_remediation_roadmap/type")
            .and_then(Value::as_str),
        Some("array"),
        "report audit remediation roadmap should be an optional array"
    );
    assert_eq!(
        schema
            .pointer("/properties/audit_remediation_roadmap/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/audit_remediation_item"),
        "report audit remediation roadmap should use its shared row fragment"
    );
    let item = required_schema_pointer("report", &schema, "/$defs/audit_remediation_item");
    assert_eq!(
        item.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "report audit remediation roadmap rows should reject unknown fields"
    );
    assert_required_fields(
        "report audit remediation roadmap item",
        item,
        &["signal", "count", "command"],
    );
    assert_enum_equals(
        "report audit remediation roadmap signal",
        &schema,
        "/$defs/audit_remediation_item/properties/signal/enum",
        &[
            "new_unreceipted",
            "expired",
            "review_due",
            "stale",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "missing_evidence",
            "broken_evidence_links",
            "weak_evidence_references",
            "baseline_debt",
            "occurrence_headroom",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/audit_remediation_item/properties/label/type")
            .and_then(Value::as_str),
        Some("string"),
        "report audit remediation roadmap label should be a string"
    );
    assert_enum_equals(
        "report audit remediation roadmap route kind",
        &schema,
        "/$defs/audit_remediation_item/properties/route_kind/enum",
        &[
            "worklist_status",
            "worklist_item_kind",
            "worklist_filter",
            "prune_stale",
        ],
    );
    assert_enum_equals(
        "report audit remediation roadmap item kind",
        &schema,
        "/$defs/audit_remediation_item/properties/item_kind/enum",
        &[
            "new_unreceipted_finding",
            "expired_allow",
            "review_due",
            "stale_allow",
            "ambiguous_selector",
            "invalid_selector",
            "missing_required_field",
            "missing_evidence",
            "broken_evidence_link",
            "weak_evidence_reference",
            "baseline_debt",
            "occurrence_headroom",
        ],
    );
    assert_enum_equals(
        "report audit remediation roadmap worklist status",
        &schema,
        "/$defs/audit_remediation_item/properties/worklist_status/enum",
        &[
            "new",
            "expired",
            "review_due",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
        ],
    );
    assert_enum_equals(
        "report audit remediation roadmap worklist filter",
        &schema,
        "/$defs/audit_remediation_item/properties/worklist_filter/enum",
        &["missing_evidence", "baseline_debt"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/audit_remediation_item/properties/count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "report audit remediation roadmap count should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/$defs/audit_remediation_item/properties/count/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "report audit remediation roadmap count should be non-negative"
    );
    assert_eq!(
        schema
            .pointer("/$defs/audit_remediation_item/properties/command/type")
            .and_then(Value::as_str),
        Some("string"),
        "report audit remediation roadmap command should be a string"
    );
}

#[test]
fn report_schema_constrains_audit_remediation_roadmap_to_audit_command() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let all_of = schema
        .get("allOf")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("report schema allOf should be an array"));
    let has_audit_constraint = all_of.iter().any(|constraint| {
        constraint
            .pointer("/if/required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                required
                    .iter()
                    .any(|field| field.as_str() == Some("audit_remediation_roadmap"))
            })
            && constraint
                .pointer("/then/properties/command/const")
                .and_then(Value::as_str)
                == Some("audit")
    });

    assert!(
        has_audit_constraint,
        "report audit_remediation_roadmap should be constrained to command=audit"
    );
}

#[test]
fn report_schema_allows_optional_policy_missing_evidence_counts() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    for pointer in [
        "/$defs/summary/properties/policy_missing_evidence",
        "/$defs/trend/properties/policy_missing_evidence",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }
}

#[test]
fn report_schema_allows_optional_source_inventory() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let source_inventory =
        required_schema_pointer("report", &schema, "/properties/source_inventory/$ref");
    assert_eq!(
        source_inventory.as_str(),
        Some("#/$defs/source_inventory"),
        "report source_inventory should use the shared source inventory fragment"
    );

    for pointer in [
        "/$defs/source_inventory/properties/findings",
        "/$defs/source_inventory_kind_row/properties/total",
        "/$defs/source_inventory_kind_row/properties/matched",
        "/$defs/source_inventory_kind_row/properties/new",
        "/$defs/source_inventory_kind_row/properties/review_items",
        "/$defs/source_inventory_family_row/properties/total",
        "/$defs/source_inventory_family_row/properties/matched",
        "/$defs/source_inventory_family_row/properties/new",
        "/$defs/source_inventory_family_row/properties/review_items",
    ] {
        let count = required_schema_pointer("report", &schema, pointer);
        assert_eq!(
            count.get("type").and_then(Value::as_str),
            Some("integer"),
            "report {pointer} count type"
        );
        assert_eq!(
            count.get("minimum").and_then(Value::as_u64),
            Some(0),
            "report {pointer} count minimum"
        );
    }

    for pointer in [
        "/$defs/source_inventory_kind_row/properties/kind/$ref",
        "/$defs/source_inventory_family_row/properties/kind/$ref",
    ] {
        assert_eq!(
            required_schema_pointer("report", &schema, pointer).as_str(),
            Some("#/$defs/governed_source_exception_kind"),
            "report {pointer} kind vocabulary"
        );
    }
}
