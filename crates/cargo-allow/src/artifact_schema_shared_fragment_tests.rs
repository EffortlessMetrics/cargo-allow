use crate::artifact_schema_support::{parse_schema, required_schema_pointer};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[test]
fn common_schema_fragment_catalog_keeps_expected_defs() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("common schema $defs should be an object");
    };
    let actual = defs.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = [
        "allow_entry",
        "audit_remediation_item",
        "canonical_evidence_prefix",
        "claim_boundary_flag",
        "counts",
        "current_finding",
        "diff",
        "diff_analysis",
        "diff_movement_counts",
        "diff_posture_delta_counts",
        "diff_summary",
        "evidence_change",
        "evidence_change_field",
        "evidence_reference",
        "evidence_reference_category",
        "evidence_reference_status",
        "exception_identity_change",
        "exception_identity_change_field",
        "finding",
        "finding_posture_change",
        "finding_posture_kind",
        "governed_source_exception_kind",
        "inventory",
        "inventory_source",
        "lifecycle",
        "local_file_evidence_prefix",
        "lifecycle_change",
        "lifecycle_change_field",
        "match_status",
        "metadata_change",
        "metadata_change_field",
        "occurrence_limit_change",
        "outcome",
        "policy_change",
        "policy_change_kind",
        "policy_change_severity",
        "policy_status_change",
        "policy_migration_inventory",
        "recognized_evidence_prefix",
        "requirement_change",
        "requirement_change_field",
        "scanner_limitation",
        "selector",
        "selector_identity_change",
        "selector_identity_change_field",
        "selector_precision_change",
        "selector_precision_field",
        "selected_finding",
        "scope_change",
        "scope_change_field",
        "span",
        "stale_entry",
        "summary",
        "structural_identity",
        "source_syntax_inventory",
        "source_inventory",
        "source_inventory_family_row",
        "source_inventory_kind_row",
        "trend",
        "traceability_evidence_prefix",
        "work_item",
        "worklist_filters",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(
        actual, expected,
        "common.v1 shared fragment names are part of the schema compatibility surface"
    );
}

#[test]
fn report_schema_fragments_are_mirrored_in_common_catalog() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let Some(common_defs) = common.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("common schema $defs should be an object");
    };
    let Some(report_defs) = report.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("report schema $defs should be an object");
    };

    let missing = report_defs
        .keys()
        .filter(|name| !common_defs.contains_key(*name))
        .map(String::as_str)
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "report.schema.json $defs should be mirrored in common.v1.json: {missing:?}"
    );
}

#[test]
fn artifact_local_fragments_match_common_wire_shapes() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );
    let receipt = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );
    let prune = parse_schema(
        "prune",
        include_str!("../../../docs/schemas/prune.schema.json"),
    );
    let add = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));
    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );
    let worklist = parse_schema(
        "worklist",
        include_str!("../../../docs/schemas/worklist.schema.json"),
    );

    for fragment in [
        "diff",
        "diff_movement_counts",
        "diff_posture_delta_counts",
        "diff_summary",
        "finding",
        "inventory",
        "outcome",
        "summary",
        "trend",
        "source_inventory",
        "audit_remediation_item",
        "source_inventory_kind_row",
        "source_inventory_family_row",
        "structural_identity",
        "finding_posture_change",
        "policy_change",
        "selector_precision_field",
        "selector_precision_change",
        "scope_change_field",
        "scope_change",
        "occurrence_limit_change",
        "lifecycle_change_field",
        "lifecycle_change",
        "evidence_change_field",
        "evidence_change",
        "exception_identity_change_field",
        "exception_identity_change",
        "selector_identity_change_field",
        "selector_identity_change",
        "metadata_change_field",
        "metadata_change",
        "requirement_change_field",
        "requirement_change",
        "policy_status_change",
    ] {
        assert_common_fragment_matches("report", &report, &common, fragment);
    }

    for (schema_name, schema) in [("add", &add), ("explain", &explain)] {
        assert_common_fragment_matches(schema_name, schema, &common, "structural_identity");
        assert_common_fragment_matches(schema_name, schema, &common, "selector");
    }

    assert_common_fragment_matches_named("add", &add, "finding", &common, "selected_finding");
    assert_common_fragment_matches_named(
        "explain",
        &explain,
        "current_finding",
        &common,
        "current_finding",
    );

    for (schema_name, schema) in [("explain", &explain), ("worklist", &worklist)] {
        assert_common_fragment_matches(schema_name, schema, &common, "evidence_reference");
        assert_common_fragment_matches(schema_name, schema, &common, "evidence_reference_category");
        assert_common_fragment_matches(schema_name, schema, &common, "evidence_reference_status");
    }

    assert_common_fragment_matches_named(
        "worklist",
        &worklist,
        "filters",
        &common,
        "worklist_filters",
    );
    assert_common_fragment_matches("worklist", &worklist, &common, "work_item");

    for fragment in ["allow_entry", "lifecycle", "span"] {
        assert_common_fragment_matches("explain", &explain, &common, fragment);
    }

    for fragment in [
        "counts",
        "source_inventory",
        "source_inventory_kind_row",
        "source_inventory_family_row",
    ] {
        assert_common_fragment_matches("receipt", &receipt, &common, fragment);
    }
    assert_common_fragment_matches("prune", &prune, &common, "stale_entry");
    assert_common_fragment_matches_named("explain", &explain, "match_outcome", &common, "outcome");
}

fn assert_common_fragment_matches(
    schema_name: &str,
    schema: &Value,
    common: &Value,
    fragment: &str,
) {
    assert_common_fragment_matches_named(schema_name, schema, fragment, common, fragment);
}

fn assert_common_fragment_matches_named(
    schema_name: &str,
    schema: &Value,
    schema_fragment: &str,
    common: &Value,
    common_fragment: &str,
) {
    let schema_pointer = format!("/$defs/{schema_fragment}");
    let common_pointer = format!("/$defs/{common_fragment}");
    let schema_fragment = required_schema_pointer(schema_name, schema, &schema_pointer);
    let common_fragment = required_schema_pointer("common", common, &common_pointer);
    assert_eq!(
        schema_wire_shape(schema_fragment),
        schema_wire_shape(common_fragment),
        "{schema_name} {schema_pointer} should match common.v1 {common_pointer} wire shape"
    );
}

fn schema_wire_shape(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut clean = Map::new();
            for (key, child) in object {
                if key != "description" {
                    clean.insert(key.clone(), schema_wire_shape(child));
                }
            }
            Value::Object(clean)
        }
        Value::Array(items) => Value::Array(items.iter().map(schema_wire_shape).collect()),
        _ => value.clone(),
    }
}
